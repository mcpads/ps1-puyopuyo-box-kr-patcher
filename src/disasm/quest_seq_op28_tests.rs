use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];

    for (index, handler) in QUEST_SEQ_OP28_DISPATCH_HANDLERS.iter().enumerate() {
        let address = QUEST_SEQ_OP28_DISPATCH_TABLE_RUNTIME + u32::try_from(index).unwrap() * 4;
        let offset = map.text_offset(address).expect("dispatch table 범위");
        text[offset..offset + 4].copy_from_slice(&handler.to_le_bytes());
    }
    for site in QUEST_SEQ_OP28_DIRECT_ADVANCE_SITES {
        for (address, kind) in [
            (
                site.load_runtime,
                K::Lw {
                    rt: site.source_reg,
                    base: site.object_reg,
                    offset: 0x1C,
                },
            ),
            (
                site.increment_runtime,
                K::Addiu {
                    rt: site.value_reg,
                    rs: site.source_reg,
                    imm: site.record_bytes,
                },
            ),
            (
                site.store_runtime,
                K::Sw {
                    rt: site.value_reg,
                    base: site.object_reg,
                    offset: 0x1C,
                },
            ),
        ] {
            let offset = map.text_offset(address).expect("direct advance 범위");
            text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
        }
    }
    for (address, kind) in dispatcher_and_branch_fixture() {
        let offset = map.text_offset(address).expect("dispatcher/branch 범위");
        text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
    }
    (map, text)
}

fn dispatcher_and_branch_fixture() -> Vec<(u32, K)> {
    let mut instructions = vec![
        (
            0x8002_30FC,
            K::Jal {
                target: QUEST_SEQ_OP28_DISPATCH_RUNTIME,
            },
        ),
        (
            0x8001_B0B0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8001_B0B4,
            K::Lui {
                rt: Reg::V1,
                imm: 0x800B,
            },
        ),
        (
            0x8001_B0B8,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            0x8001_B0BC,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x1A74,
            },
        ),
        (
            0x8001_B0C0,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8001_B0C4,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8001_B0C8,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8001_B0D0,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
    ];
    for (current, flag, branch, pointer, store, yield_lui, yield_store, pointer_offset) in [
        (
            0x8001_C40C,
            0x8001_C414,
            0x8001_C41C,
            0x8001_C424,
            0x8001_C42C,
            0x8001_C430,
            0x8001_C434,
            4,
        ),
        (
            0x8001_C4A8,
            0x8001_C4B0,
            0x8001_C4B8,
            0x8001_C4C0,
            0x8001_C4C8,
            0x8001_C4CC,
            0x8001_C4D0,
            8,
        ),
    ] {
        instructions.extend([
            (
                current,
                K::Lw {
                    rt: Reg::V1,
                    base: Reg::S0,
                    offset: 0x1C,
                },
            ),
            (
                flag,
                K::Lbu {
                    rt: Reg::V0,
                    base: Reg::V1,
                    offset: 3,
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
            (
                pointer,
                K::Lw {
                    rt: Reg::V0,
                    base: Reg::V1,
                    offset: pointer_offset,
                },
            ),
            (
                store,
                K::Sw {
                    rt: Reg::V0,
                    base: Reg::S0,
                    offset: 0x1C,
                },
            ),
            (
                yield_lui,
                K::Lui {
                    rt: Reg::AT,
                    imm: 0x800C,
                },
            ),
            (
                yield_store,
                K::Sw {
                    rt: Reg::V0,
                    base: Reg::AT,
                    offset: -7008,
                },
            ),
        ]);
    }
    instructions
}

#[test]
fn op28의_32entry와_14handler_소비를_고정한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_seq_op28(map, &text),
        Ok(QuestSeqOp28Summary {
            dispatch_entries: 32,
            unique_handlers: 14,
            fallback_entries: 19,
            direct_advance_sites: 15,
            direct_advance_handlers: 14,
            branch_subopcodes: 2,
            widths: [4, 8, 12, 16],
            dispatch_runtime: 0x8001_B0A8,
            dispatch_table_runtime: 0x800B_1A74,
        })
    );
}

#[test]
fn dispatch_word_drift를_거부한다() {
    let (map, mut text) = fixture();
    let address = QUEST_SEQ_OP28_DISPATCH_TABLE_RUNTIME + 3 * 4;
    let offset = map.text_offset(address).expect("dispatch table 범위");
    text[offset..offset + 4].copy_from_slice(&0x8001_C3C8u32.to_le_bytes());
    assert_eq!(
        verify_box_quest_seq_op28(map, &text),
        Err(QuestSeqOp28Error::DispatchWordMismatch {
            subopcode: 3,
            expected: 0x8001_C518,
            actual: 0x8001_C3C8,
        })
    );
}

#[test]
fn sub01의_직접_전진폭_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8001_C49C).expect("direct advance 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 8,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_op28(map, &text),
        Err(QuestSeqOp28Error::InstructionMismatch {
            subopcode: Some(1),
            address: 0x8001_C49C,
            ..
        })
    ));
}
