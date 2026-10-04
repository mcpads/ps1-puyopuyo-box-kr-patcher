use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    let descriptor = map
        .text_offset(QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME)
        .expect("descriptor 범위");
    text[descriptor..descriptor + WAKEUP_SEQUENCE_DESCRIPTOR.len()]
        .copy_from_slice(&WAKEUP_SEQUENCE_DESCRIPTOR);
    let name = map
        .text_offset(QUEST_WAKEUP_SEQUENCE_NAME_RUNTIME)
        .expect("name 범위");
    text[name..name + WAKEUP_SEQUENCE_NAME.len()].copy_from_slice(WAKEUP_SEQUENCE_NAME);

    let instructions = [
        (
            0x8007_B554,
            K::Lui {
                rt: Reg::A1,
                imm: 0x8013,
            },
        ),
        (
            0x8007_B570,
            K::Ori {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x9200,
            },
        ),
        (
            0x8007_B574,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 8,
            },
        ),
        (
            0x8007_B578,
            K::Jal {
                target: 0x8001_B12C,
            },
        ),
        (
            0x8007_B57C,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 129,
            },
        ),
        (
            0x8007_B640,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 12,
            },
        ),
        (
            0x8007_B644,
            K::Lh {
                rt: Reg::A1,
                base: Reg::V0,
                offset: -0x36FA,
            },
        ),
        (
            0x8007_B648,
            K::Jal {
                target: 0x8009_E348,
            },
        ),
        (
            0x8007_B650,
            K::Sh {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 16,
            },
        ),
        (
            0x8007_B6B8,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::A2,
                imm: -0x3EF0,
            },
        ),
        (
            0x8007_B6C4,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 5,
            },
        ),
        (
            0x8007_B6CC,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8007_B6E0,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 2,
            },
        ),
        (
            0x8007_B6F4,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 12,
            },
        ),
        (
            0x8007_B708,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 16,
            },
        ),
        (
            0x8007_B74C,
            K::Sw {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 28,
            },
        ),
        (
            0x8007_BD58,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 18,
            },
        ),
        (
            0x8007_BD60,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            0x8007_BD7C,
            K::Jal {
                target: 0x8007_D534,
            },
        ),
        (
            0x8007_BD8C,
            K::Xori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 3,
            },
        ),
        (
            0x8007_BD90,
            K::Sb {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 18,
            },
        ),
        (
            0x8007_D5DC,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8007_D5E0,
            K::Lh {
                rt: Reg::A0,
                base: Reg::V1,
                offset: 2,
            },
        ),
        (
            0x8007_D5E4,
            K::Jal {
                target: 0x800A_08C8,
            },
        ),
        (
            0x8007_D5E8,
            K::Addu {
                rd: Reg::A2,
                rs: Reg::A1,
                rt: Reg::ZERO,
            },
        ),
        (
            0x800A_08E0,
            K::Addu {
                rd: Reg::A1,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            0x800A_08E8,
            K::Jal {
                target: 0x800A_0A18,
            },
        ),
    ];
    for (address, kind) in instructions {
        let offset = map.text_offset(address).expect("명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
    }
    (map, text)
}

#[test]
fn descriptor에서_sequence_start까지_소비_경로를_고정한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_sound_sequence_consumer(map, &text),
        Ok(QuestSoundSequenceConsumerSummary {
            profile: IsaProfile::DECLARED,
            sequence_descriptor_runtime: 0x800B_4C90,
            sequence_name_runtime: 0x8001_339C,
            sequence_bytes: 0x5B28,
            load_destination_runtime: 0x8013_9200,
            loader_function_runtime: 0x8001_B12C,
            sequence_open_runtime: 0x8009_E348,
            application_slot_runtime: 0x800F_C110,
            application_slot_stride: 0x20,
            start_controller_runtime: 0x8007_BD58,
            sequence_start_runtime: 0x8007_D5E4,
            low_level_start_wrapper_runtime: 0x800A_08C8,
            low_level_start_core_runtime: 0x800A_0A18,
        })
    );
}

#[test]
fn descriptor_size_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map
        .text_offset(QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME + 8)
        .expect("descriptor size 범위");
    text[offset] ^= 1;
    assert!(matches!(
        verify_box_quest_sound_sequence_consumer(map, &text),
        Err(QuestSoundSequenceConsumerError::OriginalBytesMismatch { .. })
    ));
}

#[test]
fn sequence_start_handle_offset_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8007_D5E0).expect("start 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Lh {
            rt: Reg::A0,
            base: Reg::V1,
            offset: 4,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_sound_sequence_consumer(map, &text),
        Err(QuestSoundSequenceConsumerError::InstructionMismatch {
            address: 0x8007_D5E0,
            ..
        })
    ));
}
