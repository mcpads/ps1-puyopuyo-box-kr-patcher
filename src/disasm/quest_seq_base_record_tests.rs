use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    for (index, handler) in QUEST_SEQ_OP3A_SUBDISPATCH_HANDLERS.iter().enumerate() {
        let address = QUEST_SEQ_OP3A_SUBDISPATCH_TABLE_RUNTIME + u32::try_from(index).unwrap() * 4;
        let offset = map.text_offset(address).expect("OP3A table 범위");
        text[offset..offset + 4].copy_from_slice(&handler.to_le_bytes());
    }
    for (address, kind) in fixture_instructions() {
        let offset = map.text_offset(address).expect("fixture 명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
    }
    (map, text)
}

fn fixture_instructions() -> Vec<(u32, K)> {
    let mut instructions = Vec::new();
    for (selector, mask, branch) in [
        (0x8002_0754, 0x8002_0A04, 0x8002_0A08),
        (0x8002_0A6C, 0x8002_0CFC, 0x8002_0D00),
    ] {
        instructions.extend([
            (
                selector,
                K::Lbu {
                    rt: Reg::S3,
                    base: Reg::A0,
                    offset: 1,
                },
            ),
            (
                mask,
                K::Andi {
                    rt: Reg::V0,
                    rs: Reg::S3,
                    imm: u16::from(QUEST_SEQ_BASE_RECORD_MODE_MASK),
                },
            ),
            (
                branch,
                K::Beq {
                    rs: Reg::V0,
                    rt: Reg::ZERO,
                    offset: 4,
                },
            ),
        ]);
    }
    instructions.extend([
        (
            0x8002_1F9C,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            0x8002_1FA4,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 25,
            },
        ),
        (
            0x8002_1FA8,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 0x136,
            },
        ),
        (
            0x8002_1FAC,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8001,
            },
        ),
        (
            0x8002_1FB0,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x4B4,
            },
        ),
        (
            0x8002_1FB4,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            0x8002_1FB8,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8002_1FBC,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (0x8002_1FC4, K::Jr { rs: Reg::V0 }),
        (
            0x8002_1FEC,
            K::J {
                target: 0x8002_2474,
            },
        ),
        (
            0x8002_2108,
            K::J {
                target: 0x8002_2474,
            },
        ),
        (
            0x8002_2144,
            K::J {
                target: 0x8002_2120,
            },
        ),
        (
            0x8002_2130,
            K::J {
                target: 0x8002_2474,
            },
        ),
        (
            0x8002_2464,
            K::J {
                target: 0x8002_2474,
            },
        ),
        (
            0x8002_2474,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
        (
            0x8002_247C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
        (
            0x8002_2480,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
    ]);
    instructions
}

#[test]
fn record_byte가_op10_11의_4_8바이트_전진을_고른다() {
    for opcode in [0x10, 0x11] {
        assert_eq!(
            quest_seq_base_record_selected_advance(opcode, 0x00),
            Some(8)
        );
        assert_eq!(
            quest_seq_base_record_selected_advance(opcode, 0xCF),
            Some(8)
        );
        assert_eq!(
            quest_seq_base_record_selected_advance(opcode, 0x10),
            Some(4)
        );
        assert_eq!(
            quest_seq_base_record_selected_advance(opcode, 0x20),
            Some(4)
        );
        assert_eq!(
            quest_seq_base_record_selected_advance(opcode, 0x30),
            Some(4)
        );
    }
}

#[test]
fn op3a_sub02_08_0a_16만_현재_정적_전진으로_승인한다() {
    for subopcode in [0x02, 0x08, 0x0A, 0x16] {
        assert_eq!(
            quest_seq_base_record_selected_advance(0x3A, subopcode),
            Some(4)
        );
    }
    assert_eq!(quest_seq_base_record_selected_advance(0x3A, 0x03), None);
}

#[test]
fn 공용_fallback_entry는_전부_4바이트다() {
    assert_eq!(quest_seq_base_fallback_advance(0x30), Some(4));
    assert_eq!(quest_seq_base_fallback_advance(0x40), Some(4));
    assert_eq!(quest_seq_base_fallback_advance(0x10), None);
}

#[test]
fn selector와_op3a_table을_typed_source로_검증한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_seq_base_record_advances(map, &text),
        Ok(QuestSeqBaseRecordSummary {
            mode_selected_opcodes: 2,
            mode_mask: 0x30,
            op3a_dispatch_entries: 25,
            op3a_static_advance_subopcodes: 4,
            fallback_entries: 82,
        })
    );
}

#[test]
fn selector와_dispatch_word_drift를_거부한다() {
    let (map, mut text) = fixture();
    let selector = map.text_offset(0x8002_0754).unwrap();
    text[selector..selector + 4].copy_from_slice(
        &encode(K::Lbu {
            rt: Reg::S3,
            base: Reg::A0,
            offset: 2,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_base_record_advances(map, &text),
        Err(QuestSeqBaseRecordError::InstructionMismatch {
            opcode: 0x10,
            address: 0x8002_0754,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let table = map
        .text_offset(QUEST_SEQ_OP3A_SUBDISPATCH_TABLE_RUNTIME)
        .unwrap();
    text[table..table + 4].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        verify_box_quest_seq_base_record_advances(map, &text),
        Err(QuestSeqBaseRecordError::DispatchWordMismatch { subopcode: 0, .. })
    ));
}
