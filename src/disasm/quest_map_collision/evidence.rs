//! QUEST QMP 충돌 경로의 원본 typed instruction 증거.

use crate::isa::{InstructionKind as K, Reg};
use crate::qmp::QMP_HEADER_LEN;

use super::{
    QUEST_COLLISION_ACTOR_DIRECTION_OFFSET, QUEST_COLLISION_ACTOR_LAYER_OFFSET,
    QUEST_COLLISION_ACTOR_STATE_OFFSET, QUEST_COLLISION_CLASSIFIER_RUNTIME,
    QUEST_COLLISION_CONDITION_WRITER_RUNTIME, QUEST_COLLISION_EXPLICIT_RANGE_END,
    QUEST_COLLISION_EXPLICIT_RANGE_START, QUEST_COLLISION_INPUT_STATE_MASK,
    QUEST_COLLISION_MOVEMENT_ADJUST_RUNTIME, QUEST_COLLISION_PLANE_OFFSET,
    QUEST_COLLISION_RECTANGLE_OVERLAP_RUNTIME, QUEST_COLLISION_SOLID_CLASS,
    QUEST_COLLISION_SPECIAL_CLASS, QUEST_COLLISION_TILE_FLAG_MASKS,
    QUEST_COLLISION_TILE_LOW_BYTE_MASK, QUEST_MAP_COLLISION_PLANE_LOADER_RUNTIME,
    QUEST_MAP_COLLISION_PUBLISH_RUNTIME, QUEST_MAP_FIRST_PLANE_LOADER_RUNTIME,
    QUEST_MAP_HALFWORD_COPY_RUNTIME, QUEST_MAP_HALFWORD_FILL_RUNTIME, QUEST_MAP_MOVEMENT_RUNTIME,
    QUEST_MAP_PLANE_CAPACITY_CELLS, QUEST_MAP_PLANE_STRIDE_BYTES,
    QUEST_MAP_SECOND_PLANE_LOADER_RUNTIME, QUEST_PLAYER_MAP_MOVEMENT_CALL_RUNTIME,
};

#[path = "evidence/classifier_model.rs"]
mod classifier_model;
#[path = "evidence/movement_geometry.rs"]
mod movement_geometry;
#[path = "evidence/normal_walk.rs"]
mod normal_walk;

fn map_plane_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        // OP85의 첫 세 MAP descriptor load는 완료 callback을 세 plane
        // loader에 같은 순서로 설치한다.
        (
            0x8007_111C,
            "load first-plane callback high",
            K::Lui {
                rt: Reg::V1,
                imm: 0x8007,
            },
        ),
        (
            0x8007_1124,
            "load first-plane callback",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: QUEST_MAP_FIRST_PLANE_LOADER_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8007_11EC,
            "load second-plane callback high",
            K::Lui {
                rt: Reg::V1,
                imm: 0x8007,
            },
        ),
        (
            0x8007_11F4,
            "load second-plane callback",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: QUEST_MAP_SECOND_PLANE_LOADER_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8007_12BC,
            "load collision-plane callback high",
            K::Lui {
                rt: Reg::V1,
                imm: 0x8007,
            },
        ),
        (
            0x8007_12C0,
            "load collision-plane callback",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: QUEST_MAP_COLLISION_PLANE_LOADER_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8007_12C8,
            "install selected QMP plane callback",
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 8,
            },
        ),
        // 첫 QMP payload는 비활성 slot의 첫 plane에 header 뒤부터 복사한다.
        (
            0x8007_6E5C,
            "prepare inactive slot selector",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8007_6E64,
            "load active slot address high",
            K::Lui {
                rt: Reg::V0,
                imm: 0x800C,
            },
        ),
        (
            0x8007_6E68,
            "load active slot",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -0x1A5C,
            },
        ),
        (
            0x8007_6E74,
            "select inactive slot",
            K::Subu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8007_6E80,
            "load width table",
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: -0x1A20,
            },
        ),
        (
            0x8007_6E88,
            "scale slot for width table",
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            0x8007_6E94,
            "store map width",
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0,
            },
        ),
        (
            0x8007_6EA8,
            "load height table",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x1A18,
            },
        ),
        (
            0x8007_6EB4,
            "store map height",
            K::Sw {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8007_6EC0,
            "multiply width and height",
            K::Mult {
                rs: Reg::A2,
                rt: Reg::A1,
            },
        ),
        (
            0x8007_6EC8,
            "load slot buffer root table",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x1A58,
            },
        ),
        (
            0x8007_6ED8,
            "load first plane destination",
            K::Lw {
                rt: Reg::A0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (0x8007_6EDC, "pass map cell count", K::Mflo { rd: Reg::A2 }),
        (
            0x8007_6EE0,
            "copy first QMP cell payload",
            K::Jal {
                target: QUEST_MAP_HALFWORD_COPY_RUNTIME,
            },
        ),
        (
            0x8007_6EE4,
            "skip QMP header",
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: QMP_HEADER_LEN as i16,
            },
        ),
        // 두 번째 QMP payload는 같은 비활성 slot의 +0x4800 plane에 복사한다.
        (
            0x8007_6F18,
            "load active second-plane slot",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -0x1A5C,
            },
        ),
        (
            0x8007_6F28,
            "select inactive second-plane slot",
            K::Subu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8007_6F4C,
            "multiply second-plane width and height",
            K::Mult {
                rs: Reg::A0,
                rt: Reg::A2,
            },
        ),
        (
            0x8007_6F6C,
            "select second-plane destination",
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: QUEST_MAP_PLANE_STRIDE_BYTES as i16,
            },
        ),
        (
            0x8007_6F74,
            "copy second QMP cell payload",
            K::Jal {
                target: QUEST_MAP_HALFWORD_COPY_RUNTIME,
            },
        ),
        (
            0x8007_6F78,
            "skip second QMP header",
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: QMP_HEADER_LEN as i16,
            },
        ),
        // 세 번째 QMP는 같은 비활성 slot의 collision plane에 적재한다.
        (
            0x8007_6FA4,
            "prepare inactive collision slot selector",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8007_6FAC,
            "load active collision slot",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -0x1A5C,
            },
        ),
        (
            0x8007_6FBC,
            "select inactive collision slot",
            K::Subu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8007_6FE0,
            "multiply collision width and height",
            K::Mult {
                rs: Reg::A0,
                rt: Reg::A2,
            },
        ),
        (
            0x8007_6FE4,
            "load collision plane offset",
            K::Ori {
                rt: Reg::A3,
                rs: Reg::ZERO,
                imm: QUEST_COLLISION_PLANE_OFFSET as u16,
            },
        ),
        (
            0x8007_6FEC,
            "load collision slot buffer roots",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x1A58,
            },
        ),
        (
            0x8007_6FFC,
            "load collision slot buffer root",
            K::Lw {
                rt: Reg::A0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            0x8007_7004,
            "select collision plane destination",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::A3,
            },
        ),
        (
            0x8007_700C,
            "copy collision QMP cell payload",
            K::Jal {
                target: QUEST_MAP_HALFWORD_COPY_RUNTIME,
            },
        ),
        (
            0x8007_7010,
            "skip collision QMP header",
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: QMP_HEADER_LEN as i16,
            },
        ),
        // 활성 slot의 세 번째 plane만 고정 collision buffer로 publish한다.
        (
            QUEST_MAP_COLLISION_PUBLISH_RUNTIME,
            "load published collision buffer address high",
            K::Lui {
                rt: Reg::S0,
                imm: 0x800F,
            },
        ),
        (
            0x8007_380C,
            "load published collision buffer",
            K::Addiu {
                rt: Reg::S0,
                rs: Reg::S0,
                imm: 0x7488,
            },
        ),
        (
            0x8007_3818,
            "clear published collision buffer",
            K::Jal {
                target: QUEST_MAP_HALFWORD_FILL_RUNTIME,
            },
        ),
        (
            0x8007_381C,
            "pass collision plane capacity to clear",
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: QUEST_MAP_PLANE_CAPACITY_CELLS as i16,
            },
        ),
        (
            0x8007_3824,
            "pass collision plane capacity to publish",
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: QUEST_MAP_PLANE_CAPACITY_CELLS as i16,
            },
        ),
        (
            0x8007_382C,
            "load published active slot",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -0x1A5C,
            },
        ),
        (
            0x8007_3834,
            "load published slot buffer roots",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -0x1A58,
            },
        ),
        (
            0x8007_3844,
            "load published collision plane offset",
            K::Ori {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: QUEST_COLLISION_PLANE_OFFSET as u16,
            },
        ),
        (
            0x8007_3848,
            "publish collision plane",
            K::Jal {
                target: QUEST_MAP_HALFWORD_COPY_RUNTIME,
            },
        ),
        (
            0x8007_384C,
            "select published collision plane source",
            K::Addu {
                rd: Reg::A1,
                rs: Reg::V0,
                rt: Reg::A1,
            },
        ),
    ]
}

fn classifier_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        (
            0x8006_B92C,
            "mask tile low byte",
            K::Andi {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: QUEST_COLLISION_TILE_LOW_BYTE_MASK,
            },
        ),
        (
            0x8006_B930,
            "load solid class and explicit tile 2",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: i16::from(QUEST_COLLISION_SOLID_CLASS),
            },
        ),
        (
            0x8006_B934,
            "branch explicit tile 2",
            K::Beq {
                rs: Reg::A1,
                rt: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8006_B940,
            "mask tile 0x8000 flag",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::A2,
                imm: QUEST_COLLISION_TILE_FLAG_MASKS[1],
            },
        ),
        (
            0x8006_B944,
            "split low bytes below 6",
            K::Slti {
                rt: Reg::V0,
                rs: Reg::A1,
                imm: 6,
            },
        ),
        (
            0x8006_B954,
            "load explicit tile 102",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 102,
            },
        ),
        (
            0x8006_B958,
            "load actor layer",
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_COLLISION_ACTOR_LAYER_OFFSET,
            },
        ),
        (
            0x8006_B968,
            "return from solid class path",
            K::Jr { rs: Reg::RA },
        ),
        (
            0x8006_B96C,
            "return solid class",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: i16::from(QUEST_COLLISION_SOLID_CLASS),
            },
        ),
        (
            0x8006_B988,
            "mask tile 0x4000 flag",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::A2,
                imm: QUEST_COLLISION_TILE_FLAG_MASKS[0],
            },
        ),
        (
            0x8006_B9A4,
            "reload explicit tile 102",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 102,
            },
        ),
        (
            0x8006_B9B4,
            "load explicit tile 6",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 6,
            },
        ),
        (
            0x8006_B9C4,
            "load explicit tile 1",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8006_B9D8,
            "load explicit tile 97",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 97,
            },
        ),
        (
            0x8006_B9E0,
            "load explicit tile 99",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 99,
            },
        ),
        (
            0x8006_B9F4,
            "load explicit tile 110",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 110,
            },
        ),
        (
            0x8006_BA04,
            "load explicit tile 109",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 109,
            },
        ),
        (
            0x8006_BA18,
            "bound explicit tile range end",
            K::Slti {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: i16::from(QUEST_COLLISION_EXPLICIT_RANGE_END) + 1,
            },
        ),
        (
            0x8006_BA20,
            "bound explicit tile range start",
            K::Slti {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: i16::from(QUEST_COLLISION_EXPLICIT_RANGE_START),
            },
        ),
        (
            0x8006_BA38,
            "load first OP92 collision condition",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -0x1A04,
            },
        ),
        (
            0x8006_BAA8,
            "load second OP92 collision condition",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -0x1A03,
            },
        ),
        (
            0x8006_BAE8,
            "load actor direction",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_COLLISION_ACTOR_DIRECTION_OFFSET,
            },
        ),
        (
            0x8006_BAF8,
            "load input state",
            K::Lhu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: -0x40AE,
            },
        ),
        (
            0x8006_BB00,
            "mask input state",
            K::Andi {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: QUEST_COLLISION_INPUT_STATE_MASK,
            },
        ),
        (
            0x8006_BB4C,
            "load primary actor",
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -0x1A00,
            },
        ),
        (
            0x8006_BB54,
            "compare classifier object with primary actor",
            K::Bne {
                rs: Reg::A0,
                rt: Reg::V0,
                offset: -124,
            },
        ),
        (
            0x8006_BB74,
            "load primary actor state",
            K::Lh {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_COLLISION_ACTOR_STATE_OFFSET,
            },
        ),
        (
            0x8006_BB7C,
            "compare primary actor state with 5",
            K::Xori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 5,
            },
        ),
        (
            0x8006_BB80,
            "return special contact predicate",
            K::Jr { rs: Reg::RA },
        ),
        (
            0x8006_BB84,
            "normalize special contact class",
            K::Sltu {
                rd: Reg::V0,
                rs: Reg::ZERO,
                rt: Reg::V0,
            },
        ),
        // OP92는 두 조건 byte를 record에서 매번 공급한다.
        (
            QUEST_COLLISION_CONDITION_WRITER_RUNTIME,
            "load OP92 current record",
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8007_861C,
            "load OP92 first condition",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            0x8007_8624,
            "store OP92 first condition",
            K::Sb {
                rt: Reg::V0,
                base: Reg::AT,
                offset: -0x1A04,
            },
        ),
        (
            0x8007_8630,
            "load OP92 second condition",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 2,
            },
        ),
        (
            0x8007_8638,
            "store OP92 second condition",
            K::Sb {
                rt: Reg::V0,
                base: Reg::AT,
                offset: -0x1A03,
            },
        ),
        (
            0x8007_8644,
            "advance OP92 record",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
    ]
}

fn movement_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        (
            QUEST_PLAYER_MAP_MOVEMENT_CALL_RUNTIME,
            "call map collision from player movement",
            K::Jal {
                target: QUEST_MAP_MOVEMENT_RUNTIME,
            },
        ),
        (
            0x8006_BDBC,
            "load lookup active slot",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -0x1A5C,
            },
        ),
        (
            0x8006_BDC4,
            "load lookup width table",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -0x1A20,
            },
        ),
        (
            0x8006_BDD8,
            "multiply tile y by map width",
            K::Mult {
                rs: Reg::A0,
                rt: Reg::V0,
            },
        ),
        (
            0x8006_BDE0,
            "add tile x to row base",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::T0,
                rt: Reg::A1,
            },
        ),
        (
            0x8006_BDE4,
            "scale tile index by u16",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 1,
            },
        ),
        (
            0x8006_BDEC,
            "load published collision buffer",
            K::Addiu {
                rt: Reg::T0,
                rs: Reg::T0,
                imm: 0x7488,
            },
        ),
        (
            0x8006_BDF4,
            "read collision tile",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8006_BDFC,
            "classify collision tile",
            K::Jal {
                target: QUEST_COLLISION_CLASSIFIER_RUNTIME,
            },
        ),
        (
            0x8006_BE08,
            "skip no-collision class",
            K::Beq {
                rs: Reg::S1,
                rt: Reg::ZERO,
                offset: 507,
            },
        ),
        (
            0x8006_BE48,
            "check tile rectangle overlap",
            K::Jal {
                target: QUEST_COLLISION_RECTANGLE_OVERLAP_RUNTIME,
            },
        ),
        (
            0x8006_BE74,
            "adjust movement against tile rectangle",
            K::Jal {
                target: QUEST_COLLISION_MOVEMENT_ADJUST_RUNTIME,
            },
        ),
        (
            0x8006_BE7C,
            "load special contact class",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: i16::from(QUEST_COLLISION_SPECIAL_CLASS),
            },
        ),
        (
            0x8006_BE80,
            "branch unless special contact class",
            K::Bne {
                rs: Reg::S1,
                rt: Reg::V0,
                offset: 16,
            },
        ),
        (
            0x8006_BE90,
            "increment special contact counter",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            0x8006_BFC8,
            "classify adjacent collision tile",
            K::Jal {
                target: QUEST_COLLISION_CLASSIFIER_RUNTIME,
            },
        ),
        (
            0x8006_BFD4,
            "compare adjacent tile with solid class",
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: i16::from(QUEST_COLLISION_SOLID_CLASS),
            },
        ),
        (
            0x8006_BFD8,
            "normalize adjacent solid predicate",
            K::Xori {
                rt: Reg::S0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
    ]
}

pub(super) fn required_instructions() -> Vec<(u32, &'static str, K)> {
    map_plane_instructions()
        .into_iter()
        .chain(classifier_instructions())
        .chain(classifier_model::instructions())
        .chain(movement_instructions())
        .chain(movement_geometry::instructions())
        .chain(normal_walk::instructions())
        .collect()
}

pub(super) fn required_data_words() -> Vec<(u32, &'static str, u32)> {
    normal_walk::data_words()
}
