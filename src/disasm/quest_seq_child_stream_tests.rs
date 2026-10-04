use super::*;
use crate::coords::EXE_MAPS;
use crate::isa::{encode, InstructionKind as K};

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = EXE_MAPS
        .iter()
        .find(|map| map.name == "QUEST.EXE")
        .expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    let instructions = [
        (
            0x8007_2870,
            K::Lw {
                rt: Reg::V0,
                base: Reg(19),
                offset: 0x1C,
            },
        ),
        (
            0x8007_2878,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            0x8007_2880,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 27,
            },
        ),
        (
            0x8007_29E0,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8006,
            },
        ),
        (
            0x8007_29E4,
            K::J {
                target: 0x8007_3294,
            },
        ),
        (
            0x8007_3294,
            K::Jal {
                target: QUEST_ALLOCATE_OBJECT_RUNTIME,
            },
        ),
        (
            0x8007_329C,
            K::Addu {
                rd: Reg(17),
                rs: Reg::V0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8007_3350,
            K::Lw {
                rt: Reg::V0,
                base: Reg(19),
                offset: 0x1C,
            },
        ),
        (
            0x8007_3358,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8007_3360,
            K::Sw {
                rt: Reg::V0,
                base: Reg(17),
                offset: 0x1C,
            },
        ),
        (
            0x8007_3364,
            K::Sw {
                rt: Reg::V0,
                base: Reg(17),
                offset: 0xD4,
            },
        ),
        (
            0x8007_3370,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 12,
            },
        ),
        (
            0x8007_3374,
            K::Sw {
                rt: Reg::V0,
                base: Reg(19),
                offset: 0x1C,
            },
        ),
    ];
    for (address, kind) in instructions {
        let offset = map.text_offset(address).expect("fixture 명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
    }
    let table_offset = map
        .text_offset(QUEST_SEQ_CHILD_STREAM_SUBTYPE2_TABLE_RUNTIME)
        .expect("fixture table 범위");
    text[table_offset..table_offset + 4]
        .copy_from_slice(&QUEST_SEQ_CHILD_STREAM_SUBTYPE2_ARM_RUNTIME.to_le_bytes());
    let dispatch_runtime = 0x800B_36F0 + u32::from(QUEST_SEQ_CHILD_STREAM_OPCODE - 0x80) * 4;
    let dispatch_offset = map
        .text_offset(dispatch_runtime)
        .expect("fixture dispatch 범위");
    text[dispatch_offset..dispatch_offset + 4]
        .copy_from_slice(&QUEST_SEQ_CHILD_STREAM_HANDLER_RUNTIME.to_le_bytes());
    (map, text)
}

#[test]
fn subtype2_child_stream_설치를_검증한다() {
    let (map, text) = fixture();
    let summary = verify_box_quest_seq_child_stream(map, &text).unwrap();
    assert_eq!(summary.opcode, 0x82);
    assert_eq!(summary.record_bytes, 12);
    assert_eq!(summary.pointer_offset, 8);
    assert_eq!(summary.subtype2_arm_runtime, 0x8007_29E0);
    assert_eq!(summary.child_stream_store_runtime, 0x8007_3360);
}

#[test]
fn child_stream_pointer_load_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8007_3358).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(&0u32.to_le_bytes());
    assert!(verify_box_quest_seq_child_stream(map, &text).is_err());
}
