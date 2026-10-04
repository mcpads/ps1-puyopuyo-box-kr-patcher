//! QUEST resource slot 등록과 OP85 자산 선택·적재 경로.
//!
//! 상위 pointer consumer 검증은 이 모듈이 반환한 결과만 report에 결합한다.
//! slot 등록, TOWN00 선두 OP85 state, MAP/GRP descriptor 소비를 다른 opcode
//! pointer 의미와 섞지 않는다.

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::TOWN00_SEQ_RUNTIME_BASE;

use super::{
    require, require_word, QuestSeqPointerConsumerError, QUEST_GRP_DESCRIPTOR_ROOT_RUNTIME,
    QUEST_GRP_DESCRIPTOR_SLOT_RUNTIME, QUEST_MAP_DESCRIPTOR_ROOT_RUNTIME,
    QUEST_MAP_DESCRIPTOR_SLOT_RUNTIME, QUEST_OP85_GRP_COPY_SOURCE_RUNTIME,
    QUEST_OP85_GRP_COPY_WORDS, QUEST_OP85_GRP_LOADER_RUNTIME, QUEST_OP85_GRP_STAGING_RUNTIME,
    QUEST_OP85_MAP_LOADER_RUNTIME, QUEST_SEQ_DESCRIPTOR_ROOT_RUNTIME,
    QUEST_SEQ_DESCRIPTOR_SLOT_RUNTIME, QUEST_SEQ_OP85_CONSUMER_RUNTIME,
    QUEST_SEQ_OP85_HALFWORD_STATE_OFFSETS, QUEST_SEQ_OP85_RECORD_STRIDE,
    QUEST_SEQ_OP85_RENDER_POINTER_STATE_OFFSETS, QUEST_SEQ_OP85_SELECTOR_STATE_OFFSET,
    QUEST_SEQ_OP85_STATE_RUNTIME, QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct QuestSeqResourceLoadingSummary {
    pub(super) prefix_entry_runtime: u32,
    pub(super) op85_consumer_runtime: u32,
    pub(super) op85_state_runtime: u32,
    pub(super) op85_record_stride: i16,
    pub(super) op85_selector_state_offset: i16,
    pub(super) op85_halfword_state_offsets: [i16; 5],
    pub(super) op85_render_pointer_state_offsets: [i16; 2],
    pub(super) grp_descriptor_slot_runtime: u32,
    pub(super) seq_descriptor_slot_runtime: u32,
    pub(super) map_descriptor_slot_runtime: u32,
    pub(super) grp_descriptor_root_runtime: u32,
    pub(super) seq_descriptor_root_runtime: u32,
    pub(super) map_descriptor_root_runtime: u32,
    pub(super) op85_map_loader_runtime: u32,
    pub(super) op85_grp_loader_runtime: u32,
    pub(super) op85_grp_staging_runtime: [u32; 2],
    pub(super) op85_grp_copy_source_runtime: [u32; 2],
    pub(super) op85_grp_copy_words: [u32; 2],
}

pub(super) fn verify_quest_seq_resource_loading(
    instructions: &[Instruction],
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqResourceLoadingSummary, QuestSeqPointerConsumerError> {
    verify_quest_resource_slots(instructions, map, text)?;
    verify_town00_prefix_consumer(instructions, map)?;
    verify_op85_resource_consumers(instructions, map)?;

    Ok(QuestSeqResourceLoadingSummary {
        prefix_entry_runtime: TOWN00_SEQ_RUNTIME_BASE,
        op85_consumer_runtime: QUEST_SEQ_OP85_CONSUMER_RUNTIME,
        op85_state_runtime: QUEST_SEQ_OP85_STATE_RUNTIME,
        op85_record_stride: QUEST_SEQ_OP85_RECORD_STRIDE,
        op85_selector_state_offset: QUEST_SEQ_OP85_SELECTOR_STATE_OFFSET,
        op85_halfword_state_offsets: QUEST_SEQ_OP85_HALFWORD_STATE_OFFSETS,
        op85_render_pointer_state_offsets: QUEST_SEQ_OP85_RENDER_POINTER_STATE_OFFSETS,
        grp_descriptor_slot_runtime: QUEST_GRP_DESCRIPTOR_SLOT_RUNTIME,
        seq_descriptor_slot_runtime: QUEST_SEQ_DESCRIPTOR_SLOT_RUNTIME,
        map_descriptor_slot_runtime: QUEST_MAP_DESCRIPTOR_SLOT_RUNTIME,
        grp_descriptor_root_runtime: QUEST_GRP_DESCRIPTOR_ROOT_RUNTIME,
        seq_descriptor_root_runtime: QUEST_SEQ_DESCRIPTOR_ROOT_RUNTIME,
        map_descriptor_root_runtime: QUEST_MAP_DESCRIPTOR_ROOT_RUNTIME,
        op85_map_loader_runtime: QUEST_OP85_MAP_LOADER_RUNTIME,
        op85_grp_loader_runtime: QUEST_OP85_GRP_LOADER_RUNTIME,
        op85_grp_staging_runtime: QUEST_OP85_GRP_STAGING_RUNTIME,
        op85_grp_copy_source_runtime: QUEST_OP85_GRP_COPY_SOURCE_RUNTIME,
        op85_grp_copy_words: QUEST_OP85_GRP_COPY_WORDS,
    })
}

fn verify_quest_resource_slots(
    instructions: &[Instruction],
    map: &ExeMap,
    text: &[u8],
) -> Result<(), QuestSeqPointerConsumerError> {
    // resource 등록 record는 byte +1을 slot index, u32 +4를 36바이트
    // descriptor root로 사용한다.
    for (address, expected, kind) in [
        (
            0x8002_3120,
            "lui v1, 0x800E",
            K::Lui {
                rt: Reg::V1,
                imm: 0x800E,
            },
        ),
        (
            0x8002_312C,
            "addiu v1, v1, -0x4CD8",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -0x4CD8,
            },
        ),
        (
            0x8002_3130,
            "lbu v0, 1(a0)",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 1,
            },
        ),
        (
            0x8002_3134,
            "lw a0, 4(a0)",
            K::Lw {
                rt: Reg::A0,
                base: Reg::A0,
                offset: 4,
            },
        ),
        (
            0x8002_3138,
            "sll v0, v0, 2",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8002_313C,
            "addu v0, v0, v1",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8002_3140,
            "sw a0, 0(v0)",
            K::Sw {
                rt: Reg::A0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_3188,
            "addiu v0, v0, 8",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
    ] {
        require(instructions, map, address, expected, kind)?;
    }

    // 정적 등록 record는 slot 0/2/1을 각각 RPG_GRP/MAP/SEQ root에
    // 연결한다.
    for (address, value) in [
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
        require_word(text, map, address, value)?;
    }

    Ok(())
}

fn verify_town00_prefix_consumer(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(), QuestSeqPointerConsumerError> {
    let s1 = Reg(17);

    // 두 loader 분기가 현재 stream을 TOWN00 active base, 즉 선두 첫 OP85
    // record로 직접 설정한다.
    for (lui, ori, store, stream_reg) in [
        (0x8007_3C68, 0x8007_3C74, 0x8007_3C88, Reg::A0),
        (0x8007_3CDC, 0x8007_3CF4, 0x8007_3D04, Reg::A1),
    ] {
        require(
            instructions,
            map,
            lui,
            "lui stream, 0x8019",
            K::Lui {
                rt: stream_reg,
                imm: 0x8019,
            },
        )?;
        require(
            instructions,
            map,
            ori,
            "ori stream, stream, 0x9A70",
            K::Ori {
                rt: stream_reg,
                rs: stream_reg,
                imm: 0x9A70,
            },
        )?;
        require(
            instructions,
            map,
            store,
            "sw stream, 0x1C(s1)",
            K::Sw {
                rt: stream_reg,
                base: s1,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        )?;
    }

    // OP85는 byte +1을 state +0x0E에 저장한다. 값이 0이면
    // 0x800FBD90의 기본 halfword를 대신 저장한다.
    for (address, expected, kind) in [
        (
            0x8007_8280,
            "lui v0, 0x8010",
            K::Lui {
                rt: Reg::V0,
                imm: 0x8010,
            },
        ),
        (
            0x8007_8284,
            "addiu a1, v0, -0x4260",
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::V0,
                imm: -0x4260,
            },
        ),
        (
            0x8007_8290,
            "lw v1, 0x1C(a0)",
            K::Lw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8298,
            "lbu v0, 1(v1)",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 1,
            },
        ),
        (
            0x8007_82A0,
            "beq v0, zero, default-selector",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 4,
            },
        ),
        (
            0x8007_82B0,
            "sh v0, 0x0E(a1)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_SEQ_OP85_SELECTOR_STATE_OFFSET,
            },
        ),
        (
            0x8007_82B4,
            "lhu v0, -0x4270(v0)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -0x4270,
            },
        ),
        (
            0x8007_82BC,
            "sh v0, 0x0E(a1)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_SEQ_OP85_SELECTOR_STATE_OFFSET,
            },
        ),
        (
            0x8007_82C0,
            "lw v0, 0x1C(a0)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_82C4,
            "lui v1, 0x8010",
            K::Lui {
                rt: Reg::V1,
                imm: 0x8010,
            },
        ),
        (
            0x8007_82CC,
            "addiu v1, v1, -0x4260",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -0x4260,
            },
        ),
    ] {
        require(instructions, map, address, expected, kind)?;
    }

    // record +2..+0x0A의 five u16을 state +0x10..+0x18에 복사한다.
    for (load_runtime, record_offset, store_runtime, state_offset) in [
        (0x8007_82C8, 0x02, 0x8007_82D0, 0x10),
        (0x8007_82DC, 0x04, 0x8007_82E4, 0x12),
        (0x8007_82F0, 0x06, 0x8007_82F8, 0x14),
        (0x8007_8304, 0x08, 0x8007_830C, 0x16),
        (0x8007_8318, 0x0A, 0x8007_8320, 0x18),
    ] {
        require(
            instructions,
            map,
            load_runtime,
            "lhu v0, record-halfword(v0)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: record_offset,
            },
        )?;
        require(
            instructions,
            map,
            store_runtime,
            "sh v0, state-halfword(v1)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::V1,
                offset: state_offset,
            },
        )?;
    }

    Ok(())
}

fn verify_op85_resource_consumers(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(), QuestSeqPointerConsumerError> {
    // 첫 세 u16은 current state +4/+6/+8을 거쳐 RPG_MAP slot 2의
    // 36바이트 descriptor를 선택하고 공용 비동기 loader에 넘긴다.
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
        require(
            instructions,
            map,
            id_load,
            "lhu v1, map-id(s0)",
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S0,
                offset: state_offset,
            },
        )?;
        require(
            instructions,
            map,
            table_load,
            "lw a0, RPG_MAP-slot(a0)",
            K::Lw {
                rt: Reg::A0,
                base: Reg::A0,
                offset: -0x4CD0,
            },
        )?;
        require(
            instructions,
            map,
            shift_three,
            "sll v0, v1, 3",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V1,
                sa: 3,
            },
        )?;
        require(
            instructions,
            map,
            add_id,
            "addu v0, v0, v1",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        )?;
        require(
            instructions,
            map,
            shift_four,
            "sll v0, v0, 2",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        )?;
        require(
            instructions,
            map,
            add_descriptor,
            "addu a0, a0, v0",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::V0,
            },
        )?;
        require(
            instructions,
            map,
            call,
            "jal 0x8001B12C",
            K::Jal {
                target: QUEST_OP85_MAP_LOADER_RUNTIME,
            },
        )?;
        require(
            instructions,
            map,
            store,
            "sw a0, 0x28(s0)",
            K::Sw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x28,
            },
        )?;
    }

    // 마지막 두 u16은 RPG_GRP slot 0의 descriptor를 선택해 서로 다른
    // staging buffer로 해제한다.
    for (lui, ori, id_load, state_offset, high, low) in [
        (0x8007_1374, 0x8007_1378, 0x8007_1384, 0x0A, 0x801D, 0x9270),
        (0x8007_13FC, 0x8007_1400, 0x8007_1410, 0x0C, 0x801F, 0x1270),
    ] {
        require(
            instructions,
            map,
            lui,
            "lui a1, staging-high",
            K::Lui {
                rt: Reg::A1,
                imm: high,
            },
        )?;
        require(
            instructions,
            map,
            ori,
            "ori a1, a1, staging-low",
            K::Ori {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: low,
            },
        )?;
        require(
            instructions,
            map,
            id_load,
            "lhu v1, grp-id(s0)",
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S0,
                offset: state_offset,
            },
        )?;
    }
    for (address, expected, kind) in [
        (
            0x8007_1414,
            "lw a0, RPG_GRP-slot(a0)",
            K::Lw {
                rt: Reg::A0,
                base: Reg::A0,
                offset: -0x4CD8,
            },
        ),
        (
            0x8007_1418,
            "sll v0, v1, 3",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V1,
                sa: 3,
            },
        ),
        (
            0x8007_141C,
            "addu v0, v0, v1",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8007_1420,
            "sll v0, v0, 2",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8007_1424,
            "addu a0, a0, v0",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::V0,
            },
        ),
        (
            0x8007_1428,
            "jal 0x8001B1D4",
            K::Jal {
                target: QUEST_OP85_GRP_LOADER_RUNTIME,
            },
        ),
        (
            0x8007_142C,
            "sw a0, 0x28(s0)",
            K::Sw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x28,
            },
        ),
    ] {
        require(instructions, map, address, expected, kind)?;
    }

    // 별도 후속 경로는 고정 source를 a1, GRP staging을 a0로 두고
    // 0x6000/0x2000개의 u32를 staging 쪽으로 복사한다. loader가 staging에
    // 적재한 직후의 반대 방향으로 해석하지 않는다.
    for (
        destination_lui,
        destination_ori,
        source_lui,
        source_ori,
        call,
        words,
        destination,
        source,
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
        ),
    ] {
        let destination_high = u16::try_from(destination >> 16).expect("destination high");
        let destination_low = u16::try_from(destination & 0xFFFF).expect("destination low");
        let source_high = u16::try_from(source >> 16).expect("source high");
        let source_low = u16::try_from(source & 0xFFFF).expect("source low");
        require(
            instructions,
            map,
            destination_lui,
            "lui a0, destination-high",
            K::Lui {
                rt: Reg::A0,
                imm: destination_high,
            },
        )?;
        require(
            instructions,
            map,
            destination_ori,
            "ori a0, a0, destination-low",
            K::Ori {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: destination_low,
            },
        )?;
        require(
            instructions,
            map,
            source_lui,
            "lui a1, source-high",
            K::Lui {
                rt: Reg::A1,
                imm: source_high,
            },
        )?;
        require(
            instructions,
            map,
            source_ori,
            "ori a1, a1, source-low",
            K::Ori {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: source_low,
            },
        )?;
        require(
            instructions,
            map,
            call,
            "jal 0x8007E618",
            K::Jal {
                target: 0x8007_E618,
            },
        )?;
        let copy_words = QUEST_OP85_GRP_COPY_WORDS
            [usize::from(destination == QUEST_OP85_GRP_STAGING_RUNTIME[1])];
        require(
            instructions,
            map,
            words,
            "addiu a2, zero, copy-words",
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: i16::try_from(copy_words).expect("copy words는 i16 범위"),
            },
        )?;
    }

    Ok(())
}
