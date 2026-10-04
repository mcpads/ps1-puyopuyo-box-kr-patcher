use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];

    for (index, word) in SECTION_WORDS.into_iter().enumerate() {
        let address = QUEST_HOOK_SECTION_TABLE_RUNTIME + u32::try_from(index * 4).unwrap();
        let offset = map.text_offset(address).expect("section table 범위");
        text[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }
    let last = map
        .text_offset(QUEST_HOOK_LAST_INITIALIZED_WORD_RUNTIME)
        .expect("마지막 initialized word 범위");
    text[last..last + 4].copy_from_slice(&LAST_INITIALIZED_WORD.to_le_bytes());

    let instructions = [
        (
            0x800A_A6E8,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800C,
            },
        ),
        (
            0x800A_A6EC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x1BF0,
            },
        ),
        (
            0x800A_A6F0,
            K::Lui {
                rt: Reg::V1,
                imm: 0x8011,
            },
        ),
        (
            0x800A_A6F4,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -0x7CB8,
            },
        ),
        (
            0x800A_A6F8,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x800A_A6FC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
        (
            0x800A_A700,
            K::Sltu {
                rd: Reg::AT,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x800A_A704,
            K::Bne {
                rs: Reg::AT,
                rt: Reg::ZERO,
                offset: -4,
            },
        ),
        (
            0x800A_A708,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
        (
            GP_INITIALIZER_RUNTIME,
            K::Lui {
                rt: Reg::GP,
                imm: 0x800C,
            },
        ),
        (
            GP_INITIALIZER_RUNTIME + 4,
            K::Addiu {
                rt: Reg::GP,
                rs: Reg::GP,
                imm: -0x2078,
            },
        ),
        (
            MUTABLE_SELECTOR_STORE_RUNTIME,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            MUTABLE_SELECTOR_STORE_RUNTIME + 4,
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: 0x04E8,
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
fn tail_0영역은_대부분_startup_zero_clear와_겹친다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_hook_space(map, &text),
        Ok(QuestHookSpaceSummary {
            profile: IsaProfile::DECLARED,
            initialized_end_runtime: 0x800B_E40C,
            zero_clear_start_runtime: 0x800B_E410,
            zero_clear_end_runtime: 0x8010_8348,
            text_end_runtime: 0x800B_E800,
            alignment_padding_bytes: 4,
            zero_cleared_bytes: 302_904,
            zero_cleared_text_overlap_bytes: 1_008,
            gp_runtime: 0x800B_DF88,
            mutable_selector_runtime: 0x800B_E470,
            section_zero_start_runtime: 0x800B_E6B8,
            section_zero_bytes: 302_224,
        })
    );
}

#[test]
fn zero_clear_시작주소_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x800A_A6EC).expect("clear start 명령");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -0x1BEC,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_hook_space(map, &text),
        Err(QuestHookSpaceError::InstructionMismatch {
            address: 0x800A_A6EC,
            ..
        })
    ));
}

#[test]
fn tail과_mutable_global_drift를_거부한다() {
    let (map, mut text) = fixture();
    let tail = map
        .text_offset(QUEST_HOOK_INITIALIZED_END_RUNTIME)
        .expect("tail 범위");
    text[tail] = 1;
    assert_eq!(
        verify_box_quest_hook_space(map, &text),
        Err(QuestHookSpaceError::NonZeroTail {
            address: QUEST_HOOK_INITIALIZED_END_RUNTIME,
            actual: 1,
        })
    );

    let (map, mut text) = fixture();
    let store = map
        .text_offset(MUTABLE_SELECTOR_STORE_RUNTIME + 4)
        .expect("mutable store 범위");
    text[store..store + 4].copy_from_slice(
        &encode(K::Sw {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 0x04EC,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_hook_space(map, &text),
        Err(QuestHookSpaceError::InstructionMismatch {
            address: 0x8001_B120,
            ..
        })
    ));
}
