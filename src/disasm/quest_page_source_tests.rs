use crate::coords::LogicalFile;
use crate::isa::encode_all;

use super::*;

fn test_map() -> ExeMap {
    ExeMap {
        name: "QUEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x000A_E800,
        entry_pc: 0x800A_A6E8,
        file: LogicalFile::new(0, 0x000A_F000),
    }
}

fn put(text: &mut [u8], map: &ExeMap, address: u32, kind: K) {
    let offset = map.text_offset(address).unwrap();
    text[offset..offset + 4].copy_from_slice(&encode_all(&[kind]));
}

fn fixture() -> (ExeMap, Vec<u8>) {
    let map = test_map();
    let mut text = vec![0; map.text_len as usize];
    let kinds = [
        (
            0x8001_66D0,
            K::Addu {
                rd: Reg::A2,
                rs: Reg::A1,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_66D4,
            K::Lui {
                rt: Reg::V1,
                imm: 0x0258,
            },
        ),
        (
            0x8001_66E0,
            K::Ori {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x4E43,
            },
        ),
        (
            0x8001_66E4,
            K::Bne {
                rs: Reg::V0,
                rt: Reg::V1,
                offset: 31,
            },
        ),
        (
            0x8001_66E8,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_66EC,
            K::Lw {
                rt: Reg::A1,
                base: Reg::A2,
                offset: 0x0C,
            },
        ),
        (
            0x8001_66F4,
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: 0x04C4,
            },
        ),
        (
            0x8001_66FC,
            K::Sw {
                rt: Reg::A0,
                base: Reg::GP,
                offset: 0x04CC,
            },
        ),
        (
            0x8001_6704,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::GP,
                offset: 0x04C0,
            },
        ),
        (
            0x8001_6708,
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: 0x04C8,
            },
        ),
        (
            0x8001_6730,
            K::Sw {
                rt: Reg::V1,
                base: Reg::GP,
                offset: 0x04D0,
            },
        ),
        (
            0x8001_6744,
            K::Jal {
                target: QUEST_CNX_CORE_RUNTIME,
            },
        ),
        (
            0x8001_6748,
            K::Addu {
                rd: Reg::A2,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_675C,
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: -9,
            },
        ),
        (
            0x8001_6760,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8001_8DAC,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x18,
            },
        ),
        (
            0x8001_8DB0,
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: 0x14,
            },
        ),
        (
            0x8001_8DB4,
            K::Jal {
                target: QUEST_CNX_DIRECT_WRAPPER_RUNTIME,
            },
        ),
        (
            0x8001_67E0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: 0x04C0,
            },
        ),
        (
            0x8001_67E4,
            K::Lw {
                rt: Reg::V1,
                base: Reg::GP,
                offset: 0x04D0,
            },
        ),
        (
            0x8001_680C,
            K::Sw {
                rt: Reg::A0,
                base: Reg::GP,
                offset: 0x04CC,
            },
        ),
        (
            0x8001_6810,
            K::Sw {
                rt: Reg::A1,
                base: Reg::GP,
                offset: 0x04C8,
            },
        ),
        (
            0x8001_6830,
            K::Jal {
                target: QUEST_CNX_CORE_RUNTIME,
            },
        ),
        (
            0x8001_6834,
            K::Addu {
                rd: Reg::A2,
                rs: Reg::S4,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_B0F8,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8012,
            },
        ),
        (
            0x8001_B0FC,
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x1724,
            },
        ),
        (
            0x8001_B10C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: 0x0500,
            },
        ),
        (
            0x8001_BDC0,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8018,
            },
        ),
        (
            0x8001_BDC4,
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x3A00,
            },
        ),
        (
            0x8001_BDCC,
            K::Lw {
                rt: Reg::A0,
                base: Reg::GP,
                offset: 0x0500,
            },
        ),
        (
            0x8001_BDE4,
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x1724,
            },
        ),
        (
            0x8001_BDE8,
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: 0x0500,
            },
        ),
    ];
    for (address, kind) in kinds {
        put(&mut text, &map, address, kind);
    }
    (map, text)
}

#[test]
fn direct와_streaming은_같은_다섯_gp_field를_쓴다() {
    let (map, text) = fixture();
    let summary = verify_box_quest_page_source(&map, &text).unwrap();
    assert_eq!(summary.shared_gp_state_fields, 5);
    assert_eq!(summary.direct_wrapper_bytes, 0xA8);
    assert_eq!(summary.arena_bytes, 402_140);
}

#[test]
fn direct_core_call이_바뀌면_실패한다() {
    let (map, mut text) = fixture();
    put(
        &mut text,
        &map,
        0x8001_6744,
        K::Jal {
            target: 0x8001_6518,
        },
    );
    assert!(matches!(
        verify_box_quest_page_source(&map, &text),
        Err(QuestPageSourceError::InstructionMismatch {
            address: 0x8001_6744,
            ..
        })
    ));
}

#[test]
fn 다른_exe는_거부한다() {
    let (mut map, text) = fixture();
    map.name = "PUYO2.EXE";
    assert!(matches!(
        verify_box_quest_page_source(&map, &text),
        Err(QuestPageSourceError::WrongExecutable("PUYO2.EXE"))
    ));
}
