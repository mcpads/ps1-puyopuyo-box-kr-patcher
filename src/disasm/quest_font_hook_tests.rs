use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    let instructions = [
        (
            QUEST_FONT_HOOK_SITE_RUNTIME,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            QUEST_FONT_HOOK_PRESERVED_DELAY_RUNTIME,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
            K::Sw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0,
            },
        ),
        (
            0x8002_2F18,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
        (0x8002_2F1C, K::Jr { rs: Reg::RA }),
        (
            0x8002_2F20,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            QUEST_FONT_HOOK_CALLER_LOAD_RUNTIME,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_25D4,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
        (
            QUEST_FONT_HOOK_CALLER_JALR_RUNTIME,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        (
            QUEST_FONT_HOOK_CALLER_DELAY_RUNTIME,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
        (
            QUEST_FONT_HOOK_CALLER_RETURN_RUNTIME,
            K::Lw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: 0x518,
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
fn 한_word_jump와_보존_delay_tail의_abi를_고정한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_font_hook_site(map, &text),
        Ok(QuestFontHookSiteSummary {
            profile: IsaProfile::DECLARED,
            site_runtime: 0x8002_2F0C,
            overwrite_bytes: 4,
            preserved_delay_runtime: 0x8002_2F10,
            continuation_runtime: 0x8002_2F14,
            handler_words: 6,
            caller_jalr_runtime: 0x8002_25D8,
            caller_return_runtime: 0x8002_25E0,
            object_register: Reg::A0,
            return_register: Reg::RA,
            continuation_stream_register: Reg::V0,
            continuation_active_register: Reg::V1,
        })
    );
}

#[test]
fn caller_delay의_object_register_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map
        .text_offset(QUEST_FONT_HOOK_CALLER_DELAY_RUNTIME)
        .expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Addu {
            rd: Reg::A1,
            rs: Reg::S0,
            rt: Reg::ZERO,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_font_hook_site(map, &text),
        Err(QuestFontHookSiteError::InstructionMismatch {
            address: QUEST_FONT_HOOK_CALLER_DELAY_RUNTIME,
            ..
        })
    ));
}

#[test]
fn 원본_tail_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8002_2F18).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 8,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_font_hook_site(map, &text),
        Err(QuestFontHookSiteError::InstructionMismatch {
            address: 0x8002_2F18,
            ..
        })
    ));
}
