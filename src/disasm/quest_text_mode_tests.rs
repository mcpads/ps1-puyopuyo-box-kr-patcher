use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    let instructions = [
        (
            0x8002_2F0C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8002_2F10,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8002_2F14,
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
            0x8002_2580,
            K::Addiu {
                rt: Reg::S2,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8002_2598,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0,
            },
        ),
        (
            0x8002_25A0,
            K::Bne {
                rs: Reg::V0,
                rt: Reg::S2,
                offset: 5,
            },
        ),
        (
            0x8002_25A8,
            K::Jal {
                target: 0x8003_B840,
            },
        ),
        (
            0x8002_25AC,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8003_B8FC,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
        (
            0x8003_B904,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8003_B908,
            K::Ori {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0xFEFF,
            },
        ),
        (
            0x8003_B930,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        (
            0x8003_B9E8,
            K::Lhu {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8003_B9EC,
            K::Jal {
                target: 0x8002_BA40,
            },
        ),
        (
            0x8003_B9FC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 2,
            },
        ),
        (
            0x8003_BA00,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
    ];
    for (address, kind) in instructions {
        let offset = map.text_offset(address).expect("fixture 명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
    }
    let dispatch_runtime =
        QUEST_SEQ_BASE_DISPATCH_TABLE_RUNTIME + u32::from(QUEST_SEQ_TEXT_MODE_OPCODE) * 4;
    let dispatch_offset = map
        .text_offset(dispatch_runtime)
        .expect("fixture dispatch 범위");
    text[dispatch_offset..dispatch_offset + 4]
        .copy_from_slice(&QUEST_SEQ_TEXT_MODE_HANDLER_RUNTIME.to_le_bytes());
    (map, text)
}

#[test]
fn op21에서_text_consumer까지_경로를_고정한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_text_mode(map, &text),
        Ok(QuestTextModeSummary {
            opcode: 0x21,
            record_bytes: 4,
            handler_runtime: 0x8002_2F0C,
            interpreter_runtime: 0x8002_2570,
            consumer_runtime: 0x8003_B840,
            glyph_renderer_runtime: 0x8002_BA40,
            active_word_offset: 0,
            stream_pointer_offset: 0x1C,
        })
    );
}

#[test]
fn mode_store_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8002_2F14).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_text_mode(map, &text),
        Err(QuestTextModeError::InstructionMismatch {
            address: 0x8002_2F14,
            ..
        })
    ));
}
