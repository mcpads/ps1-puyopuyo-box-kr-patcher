//! QUEST player movement geometry의 typed instruction 증거.

use crate::isa::{InstructionKind as K, Reg};

use super::super::{
    QUEST_COLLISION_ACTOR_FIXED_X_OFFSET, QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET,
    QUEST_COLLISION_ACTOR_MAP_MEMBER_MASK, QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET,
    QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS, QUEST_COLLISION_FIXED_POINT_SHIFT,
    QUEST_COLLISION_MAP_BOUNDS_RUNTIMES, QUEST_COLLISION_PROBE_EXTENT_MARGIN_PIXELS,
    QUEST_COLLISION_TILE_INTERIOR_EXTENT_PIXELS, QUEST_COLLISION_TILE_INTERIOR_INSET_PIXELS,
};

fn caller_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        (
            0x8005_7E80,
            "pass player actor to map movement",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S4,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8005_7E84,
            "derive player collision rectangle pointer",
            K::Addiu {
                rt: Reg::S3,
                rs: Reg::S4,
                imm: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[0],
            },
        ),
        (
            0x8005_7E88,
            "pass player collision rectangle",
            K::Addu {
                rd: Reg::A1,
                rs: Reg::S3,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8005_7E8C,
            "derive horizontal movement delta pointer",
            K::Addiu {
                rt: Reg::S2,
                rs: Reg::SP,
                imm: 0xD8,
            },
        ),
        (
            0x8005_7E90,
            "pass horizontal movement delta pointer",
            K::Addu {
                rd: Reg::A2,
                rs: Reg::S2,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8005_7E94,
            "derive vertical movement delta pointer",
            K::Addiu {
                rt: Reg::S1,
                rs: Reg::SP,
                imm: 0xDC,
            },
        ),
        (
            0x8005_7E98,
            "pass vertical movement delta pointer",
            K::Addu {
                rd: Reg::A3,
                rs: Reg::S1,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8005_7E9C,
            "derive movement overlap output pointer",
            K::Addiu {
                rt: Reg::S0,
                rs: Reg::SP,
                imm: 0xD0,
            },
        ),
        (
            0x8005_7EA4,
            "pass movement overlap output pointer",
            K::Sw {
                rt: Reg::S0,
                base: Reg::SP,
                offset: 0x10,
            },
        ),
    ]
}

fn actor_rectangle_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        (
            0x8006_BBCC,
            "preserve horizontal delta pointer",
            K::Sw {
                rt: Reg::A2,
                base: Reg::SP,
                offset: 0x80,
            },
        ),
        (
            0x8006_BBD0,
            "preserve vertical delta pointer",
            K::Sw {
                rt: Reg::A3,
                base: Reg::SP,
                offset: 0x84,
            },
        ),
        (
            0x8006_BBD4,
            "load actor map collision membership",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::S2,
                offset: QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET,
            },
        ),
        (
            0x8006_BBDC,
            "mask actor map collision membership",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_COLLISION_ACTOR_MAP_MEMBER_MASK,
            },
        ),
        (
            0x8006_BBF4,
            "load actor fixed horizontal position",
            K::Lw {
                rt: Reg::V1,
                base: Reg::S2,
                offset: QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
            },
        ),
        (
            0x8006_BBF8,
            "load horizontal movement delta",
            K::Lw {
                rt: Reg::V0,
                base: Reg::T0,
                offset: 0,
            },
        ),
        (
            0x8006_BC10,
            "convert horizontal fixed point to pixel",
            K::Sra {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: QUEST_COLLISION_FIXED_POINT_SHIFT,
            },
        ),
        (
            0x8006_BC14,
            "load actor rectangle horizontal origin",
            K::Lhu {
                rt: Reg::V1,
                base: Reg::A1,
                offset: 0,
            },
        ),
        (
            0x8006_BC20,
            "store projected actor rectangle x",
            K::Sh {
                rt: Reg::A0,
                base: Reg::SP,
                offset: 0x18,
            },
        ),
        (
            0x8006_BC24,
            "load actor fixed vertical position",
            K::Lw {
                rt: Reg::V1,
                base: Reg::S2,
                offset: QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET,
            },
        ),
        (
            0x8006_BC44,
            "convert vertical fixed point to pixel",
            K::Sra {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: QUEST_COLLISION_FIXED_POINT_SHIFT,
            },
        ),
        (
            0x8006_BC4C,
            "store projected actor rectangle y",
            K::Sh {
                rt: Reg::A2,
                base: Reg::SP,
                offset: 0x1A,
            },
        ),
        (
            0x8006_BC50,
            "load actor rectangle horizontal extent",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[2]
                    - QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[0],
            },
        ),
        (
            0x8006_BC58,
            "store projected actor rectangle horizontal extent",
            K::Sh {
                rt: Reg::V0,
                base: Reg::SP,
                offset: 0x1C,
            },
        ),
        (
            0x8006_BC60,
            "load actor rectangle vertical extent",
            K::Lhu {
                rt: Reg::V1,
                base: Reg::A1,
                offset: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[3]
                    - QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[0],
            },
        ),
        (
            0x8006_BC6C,
            "store projected actor rectangle vertical extent",
            K::Sh {
                rt: Reg::V1,
                base: Reg::SP,
                offset: 0x1E,
            },
        ),
    ]
}

fn tile_probe_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        (
            0x8006_BC7C,
            "divide actor rectangle x by tile size",
            K::Sra {
                rd: Reg::A0,
                rt: Reg::A0,
                sa: 4,
            },
        ),
        (
            0x8006_BC90,
            "divide actor rectangle y by tile size",
            K::Srl {
                rd: Reg::S5,
                rt: Reg::V0,
                sa: 4,
            },
        ),
        (
            0x8006_BC94,
            "add horizontal probe extent margin",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: i16::from(QUEST_COLLISION_PROBE_EXTENT_MARGIN_PIXELS),
            },
        ),
        (
            0x8006_BCA8,
            "divide horizontal probe extent by tile size",
            K::Srl {
                rd: Reg::S7,
                rt: Reg::V0,
                sa: 4,
            },
        ),
        (
            0x8006_BCAC,
            "add vertical probe extent margin",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: i16::from(QUEST_COLLISION_PROBE_EXTENT_MARGIN_PIXELS),
            },
        ),
        (
            0x8006_BCBC,
            "divide vertical probe extent by tile size",
            K::Sra {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 4,
            },
        ),
        (
            0x8006_BD14,
            "scale probe tile y to pixels",
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V0,
                sa: 4,
            },
        ),
        (
            0x8006_BD18,
            "inset probe tile y",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: i16::from(QUEST_COLLISION_TILE_INTERIOR_INSET_PIXELS),
            },
        ),
        (
            0x8006_BD54,
            "load minimum probe tile x",
            K::Lh {
                rt: Reg::V1,
                base: Reg::V1,
                offset: QUEST_COLLISION_MAP_BOUNDS_RUNTIMES[0] as u16 as i16,
            },
        ),
        (
            0x8006_BD6C,
            "load maximum probe tile x",
            K::Lh {
                rt: Reg::V0,
                base: Reg::V0,
                offset: QUEST_COLLISION_MAP_BOUNDS_RUNTIMES[2] as u16 as i16,
            },
        ),
        (
            0x8006_BD84,
            "load minimum probe tile y",
            K::Lh {
                rt: Reg::V0,
                base: Reg::V0,
                offset: QUEST_COLLISION_MAP_BOUNDS_RUNTIMES[1] as u16 as i16,
            },
        ),
        (
            0x8006_BD9C,
            "load maximum probe tile y",
            K::Lh {
                rt: Reg::V0,
                base: Reg::V0,
                offset: QUEST_COLLISION_MAP_BOUNDS_RUNTIMES[3] as u16 as i16,
            },
        ),
        (
            0x8006_BE10,
            "pass projected actor rectangle to overlap",
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::SP,
                imm: 0x18,
            },
        ),
        (
            0x8006_BE14,
            "derive tile interior rectangle",
            K::Addiu {
                rt: Reg::S0,
                rs: Reg::SP,
                imm: 0x20,
            },
        ),
        (
            0x8006_BE2C,
            "inset probe tile x",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: i16::from(QUEST_COLLISION_TILE_INTERIOR_INSET_PIXELS),
            },
        ),
        (
            0x8006_BE30,
            "pass overlap output rectangle",
            K::Lw {
                rt: Reg::A2,
                base: Reg::SP,
                offset: 0x88,
            },
        ),
        (
            0x8006_BE3C,
            "load tile interior extent",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: i16::from(QUEST_COLLISION_TILE_INTERIOR_EXTENT_PIXELS),
            },
        ),
        (
            0x8006_BE40,
            "store tile horizontal extent",
            K::Sh {
                rt: Reg::V0,
                base: Reg::SP,
                offset: 0x24,
            },
        ),
        (
            0x8006_BE44,
            "store tile vertical extent",
            K::Sh {
                rt: Reg::V0,
                base: Reg::SP,
                offset: 0x26,
            },
        ),
        (
            0x8006_BE4C,
            "store tile interior y",
            K::Sh {
                rt: Reg::T0,
                base: Reg::SP,
                offset: 0x22,
            },
        ),
    ]
}

fn overlap_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        (
            0x8006_B624,
            "load first rectangle y",
            K::Lh {
                rt: Reg::A0,
                base: Reg::T0,
                offset: 2,
            },
        ),
        (
            0x8006_B628,
            "load first rectangle vertical extent",
            K::Lh {
                rt: Reg::V0,
                base: Reg::T0,
                offset: 6,
            },
        ),
        (
            0x8006_B62C,
            "load second rectangle y",
            K::Lh {
                rt: Reg::V1,
                base: Reg::T1,
                offset: 2,
            },
        ),
        (
            0x8006_B670,
            "load first rectangle x",
            K::Lh {
                rt: Reg::A0,
                base: Reg::T0,
                offset: 0,
            },
        ),
        (
            0x8006_B674,
            "load first rectangle horizontal extent",
            K::Lh {
                rt: Reg::V0,
                base: Reg::T0,
                offset: 4,
            },
        ),
        (
            0x8006_B678,
            "load second rectangle x",
            K::Lh {
                rt: Reg::V1,
                base: Reg::T1,
                offset: 0,
            },
        ),
        (
            0x8006_B6B4,
            "skip overlap output when pointer is null",
            K::Beq {
                rs: Reg::A2,
                rt: Reg::ZERO,
                offset: 63,
            },
        ),
        (
            0x8006_B6DC,
            "store second rectangle as overlap x",
            K::Sh {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 0,
            },
        ),
        (
            0x8006_B6E0,
            "store first rectangle as overlap x",
            K::Sh {
                rt: Reg::A0,
                base: Reg::A2,
                offset: 0,
            },
        ),
        (
            0x8006_B734,
            "store overlap width in pixels",
            K::Sh {
                rt: Reg::V0,
                base: Reg::A2,
                offset: 4,
            },
        ),
        (
            0x8006_B758,
            "store second rectangle as overlap y",
            K::Sh {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 2,
            },
        ),
        (
            0x8006_B75C,
            "store first rectangle as overlap y",
            K::Sh {
                rt: Reg::A0,
                base: Reg::A2,
                offset: 2,
            },
        ),
        (
            0x8006_B7B0,
            "store overlap height in pixels",
            K::Sh {
                rt: Reg::V0,
                base: Reg::A2,
                offset: 6,
            },
        ),
        (
            0x8006_B7B8,
            "return inclusive rectangle overlap",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
    ]
}

fn movement_adjust_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        (
            0x8006_B7BC,
            "load tile rectangle for movement adjustment",
            K::Lw {
                rt: Reg::T0,
                base: Reg::SP,
                offset: 0x10,
            },
        ),
        (
            0x8006_B7C0,
            "load horizontal fixed movement delta",
            K::Lw {
                rt: Reg::A1,
                base: Reg::A1,
                offset: 0,
            },
        ),
        (
            0x8006_B7C4,
            "load overlap rectangle for movement adjustment",
            K::Lw {
                rt: Reg::T2,
                base: Reg::SP,
                offset: 0x14,
            },
        ),
        (
            0x8006_B80C,
            "load overlap height for vertical adjustment",
            K::Lh {
                rt: Reg::A1,
                base: Reg::T2,
                offset: 6,
            },
        ),
        (
            0x8006_B814,
            "compare narrow vertical overlap",
            K::Slti {
                rt: Reg::V0,
                rs: Reg::A1,
                imm: 4,
            },
        ),
        (
            0x8006_B838,
            "load actor fixed vertical position for adjustment",
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET,
            },
        ),
        (
            0x8006_B844,
            "store adjusted fixed vertical position",
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET,
            },
        ),
        (
            0x8006_B874,
            "load vertical fixed movement delta",
            K::Lw {
                rt: Reg::A2,
                base: Reg::A2,
                offset: 0,
            },
        ),
        (
            0x8006_B8B8,
            "load overlap width for horizontal adjustment",
            K::Lh {
                rt: Reg::A1,
                base: Reg::T2,
                offset: 4,
            },
        ),
        (
            0x8006_B8C0,
            "compare narrow horizontal overlap",
            K::Slti {
                rt: Reg::V0,
                rs: Reg::A1,
                imm: 4,
            },
        ),
        (
            0x8006_B8E4,
            "load actor fixed horizontal position for adjustment",
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
            },
        ),
        (
            0x8006_B8F0,
            "store adjusted fixed horizontal position",
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
            },
        ),
    ]
}

pub(super) fn instructions() -> Vec<(u32, &'static str, K)> {
    caller_instructions()
        .into_iter()
        .chain(actor_rectangle_instructions())
        .chain(tile_probe_instructions())
        .chain(overlap_instructions())
        .chain(movement_adjust_instructions())
        .collect()
}
