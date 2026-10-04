use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    let instructions = [
        (
            0x8007_81D8,
            K::Lw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0xD4,
            },
        ),
        (
            0x8007_81F4,
            K::Sw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8007_3AB4,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_3ABC,
            K::Lhu {
                rt: Reg::S2,
                base: Reg::A0,
                offset: 2,
            },
        ),
        (
            0x8007_3AC4,
            K::Bne {
                rs: Reg::S2,
                rt: Reg::ZERO,
                offset: 2,
            },
        ),
        (
            0x8007_3AC8,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8010,
            },
        ),
        (
            0x8007_3ACC,
            K::Lhu {
                rt: Reg::S2,
                base: Reg::V0,
                offset: -0x4270,
            },
        ),
        (
            0x8007_3BF0,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800E,
            },
        ),
        (
            0x8007_3BF4,
            K::Sll {
                rd: Reg::A0,
                rt: Reg::S2,
                sa: 3,
            },
        ),
        (
            0x8007_3BF8,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::S2,
            },
        ),
        (
            0x8007_3BFC,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -0x4CD4,
            },
        ),
        (
            0x8007_3C00,
            K::Sll {
                rd: Reg::A0,
                rt: Reg::A0,
                sa: 2,
            },
        ),
        (
            0x8007_3C04,
            K::Jal {
                target: QUEST_SEQ_RESOURCE_READY_RUNTIME,
            },
        ),
        (
            0x8007_3C08,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::V0,
                rt: Reg::A0,
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
                base: Reg::S1,
                offset: 0x1C,
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
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_8DD8,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x14,
            },
        ),
        (
            0x8007_8DE8,
            K::Jal {
                target: QUEST_SEQ_RELEASE_OBJECT_RUNTIME,
            },
        ),
        (
            0x8007_8DF0,
            K::Jal {
                target: QUEST_SEQ_RELEASE_OBJECT_RUNTIME,
            },
        ),
        (
            0x8007_8DF4,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8007_66C4,
            K::Lh {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0xC6,
            },
        ),
        (
            0x8007_67AC,
            K::Jal {
                target: QUEST_SEQ_ALLOCATE_OBJECT_RUNTIME,
            },
        ),
        (
            0x8007_67B4,
            K::Lui {
                rt: Reg::V1,
                imm: 0x8009,
            },
        ),
        (
            0x8007_67B8,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -0x6778,
            },
        ),
        (
            0x8007_67BC,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x1C,
            },
        ),
    ];
    for (address, kind) in instructions {
        let offset = map.text_offset(address).expect("fixture 명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
    }
    (map, text)
}

#[test]
fn 직접_증가가_없는_네_opcode의_전이_의미를_고정한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_seq_non_advance_meanings(map, &text),
        Ok(QuestSeqNonAdvanceSummary {
            meanings: 4,
            active_stream_runtime: 0x8019_9A70,
            seq_descriptor_slot_runtime: 0x800D_B32C,
            descriptor_stride: 36,
            default_resource_id_runtime: 0x800F_BD90,
            resource_ready_runtime: 0x8001_B678,
            preset_stream_runtime: 0x8009_9888,
            release_object_runtime: 0x8001_4CD0,
            allocate_object_runtime: 0x8001_4B14,
        })
    );
    assert_eq!(
        QUEST_SEQ_NON_ADVANCE_SPECS.map(|spec| spec.opcode),
        QUEST_SEQ_WITHOUT_DIRECT_ADVANCE
    );
}

#[test]
fn 저장_stream_복원_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8007_81D8).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Lw {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0xD8,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_non_advance_meanings(map, &text),
        Err(QuestSeqNonAdvanceError::InstructionMismatch {
            opcode: 0x84,
            address: 0x8007_81D8,
            ..
        })
    ));
}

#[test]
fn op8a_resource_descriptor_stride_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8007_3BF4).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Sll {
            rd: Reg::A0,
            rt: Reg::S2,
            sa: 2,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_non_advance_meanings(map, &text),
        Err(QuestSeqNonAdvanceError::InstructionMismatch {
            opcode: 0x8A,
            address: 0x8007_3BF4,
            ..
        })
    ));
}

#[test]
fn op8a_runtime_default_선택_drift를_거부한다() {
    let (map, mut wrong_branch) = fixture();
    let branch_offset = map
        .text_offset(0x8007_3AC4)
        .expect("fixture 분기 명령 범위");
    wrong_branch[branch_offset..branch_offset + 4].copy_from_slice(
        &encode(K::Bne {
            rs: Reg::S2,
            rt: Reg::ZERO,
            offset: 1,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_non_advance_meanings(map, &wrong_branch),
        Err(QuestSeqNonAdvanceError::InstructionMismatch {
            opcode: 0x8A,
            address: 0x8007_3AC4,
            ..
        })
    ));

    let (map, mut wrong_default) = fixture();
    let load_offset = map
        .text_offset(0x8007_3ACC)
        .expect("fixture 기본값 load 명령 범위");
    wrong_default[load_offset..load_offset + 4].copy_from_slice(
        &encode(K::Lhu {
            rt: Reg::S2,
            base: Reg::V0,
            offset: -0x426E,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_non_advance_meanings(map, &wrong_default),
        Err(QuestSeqNonAdvanceError::InstructionMismatch {
            opcode: 0x8A,
            address: 0x8007_3ACC,
            ..
        })
    ));
}
