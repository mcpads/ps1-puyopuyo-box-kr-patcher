use crate::coords::LogicalFile;
use crate::isa::encode;

use super::*;

const TEST_TEXT_LEN: usize = 0xAE800;

fn quest_map() -> ExeMap {
    ExeMap {
        name: "QUEST.EXE",
        load_addr: 0x8001_0000,
        text_len: TEST_TEXT_LEN as u32,
        entry_pc: 0x800A_A6E8,
        file: LogicalFile::new(0, (TEST_TEXT_LEN + 0x800) as u32),
    }
}

fn place(text: &mut [u8], address: u32, kind: K) {
    let offset = usize::try_from(address - 0x8001_0000).expect("fixture offset");
    text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
}

fn valid_fixture() -> (ExeMap, Vec<u8>) {
    let map = quest_map();
    let mut text = vec![0; TEST_TEXT_LEN];
    let evidence = [
        (
            0x8003_DEEC,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: -4,
            },
        ),
        (
            0x8003_DEF0,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::A0,
                offset: 0,
            },
        ),
        (
            0x8003_DEF4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 9,
            },
        ),
        (
            0x8003_DEF8,
            K::And {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8003_DF00,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8003_BB08,
            K::Jal {
                target: QUEST_ADVANCE_LINE_RUNTIME,
            },
        ),
        (
            0x8002_C698,
            K::Lhu {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 6,
            },
        ),
        (
            0x8002_C69C,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::V1,
            },
        ),
        (
            0x8002_C6A4,
            K::Sh {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0x0A,
            },
        ),
        (
            0x8002_C6A8,
            K::Lbu {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0x13,
            },
        ),
        (
            0x8002_C6AC,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::A0,
            },
        ),
        (
            0x8002_C6B0,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x0C,
            },
        ),
        (
            0x8002_C6B8,
            K::Sb {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0x14,
            },
        ),
        (
            0x8003_DF18,
            K::Jal {
                target: QUEST_CLOSE_TEXT_WINDOW_RUNTIME,
            },
        ),
        (
            0x8002_C6F4,
            K::Andi {
                rt: Reg(17),
                rs: Reg(19),
                imm: 0xFF,
            },
        ),
        (
            0x8002_C6FC,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xDE,
            },
        ),
        (
            0x8002_C704,
            K::Bne {
                rs: Reg(17),
                rt: Reg::V0,
                offset: 3,
            },
        ),
        (
            0x8002_C70C,
            K::Jal {
                target: QUEST_RELEASE_OBJECT_RUNTIME,
            },
        ),
        (
            0x8002_C748,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 6,
            },
        ),
        (
            0x8002_C74C,
            K::Lhu {
                rt: Reg::A0,
                base: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8002_C754,
            K::Sb {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 0x1F,
            },
        ),
        (
            0x8002_C758,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 0x64,
            },
        ),
        (
            0x8002_C75C,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 0x68,
            },
        ),
        (
            0x8002_C760,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x0A,
            },
        ),
        (
            0x8002_C764,
            K::Sh {
                rt: Reg::A0,
                base: Reg::V0,
                offset: 0x0C,
            },
        ),
        (
            0x8003_DF50,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V1,
                offset: 2,
            },
        ),
        (
            0x8003_DF60,
            K::Sb {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0xDE,
            },
        ),
        (
            0x8003_BB60,
            K::Lh {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 2,
            },
        ),
        (
            0x8003_BB68,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 12,
            },
        ),
        (
            0x8003_BB6C,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x4C,
            },
        ),
        (
            0x8003_BB98,
            K::Lh {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 2,
            },
        ),
        (
            0x8003_BBA0,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 12,
            },
        ),
        (
            0x8003_BBA4,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x50,
            },
        ),
        (
            0x8003_BBD8,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x4C,
            },
        ),
        (
            0x8003_BBE0,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x90,
            },
        ),
        (
            0x8003_BC0C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x50,
            },
        ),
        (
            0x8003_BC14,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x94,
            },
        ),
        (
            0x8003_E01C,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 2,
            },
        ),
        (
            0x8003_E024,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 6,
            },
        ),
        (
            0x8003_E04C,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 2,
            },
        ),
        (
            0x8003_E054,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8003_E33C,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A1,
                offset: 2,
            },
        ),
        (
            0x8003_E340,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x14,
            },
        ),
        (
            0x8003_E348,
            K::Sltu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8003_E34C,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 2,
            },
        ),
        (
            0x8003_E354,
            K::Sb {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0x14,
            },
        ),
        (
            0x8003_E47C,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 2,
            },
        ),
        (
            0x8003_E484,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x0A,
            },
        ),
        (
            0x8003_E4AC,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 2,
            },
        ),
        (
            0x8003_E4B4,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x0C,
            },
        ),
        (
            0x8003_E500,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8003_E504,
            K::Sb {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x1F,
            },
        ),
        (
            0x8003_E550,
            K::Sb {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 0x21,
            },
        ),
        (
            0x8003_E558,
            K::Jal {
                target: QUEST_OPEN_TEXT_WINDOW_RUNTIME,
            },
        ),
        (
            0x8002_C264,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x68,
            },
        ),
        (
            0x8002_C26C,
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 23,
            },
        ),
        (
            0x8002_C278,
            K::Jal {
                target: QUEST_ALLOCATE_OBJECT_RUNTIME,
            },
        ),
        (
            0x8002_C2C8,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x68,
            },
        ),
        (
            0x8003_DDDC,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8003_DDE0,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
        (
            0x8003_DDE4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 2,
            },
        ),
        (0x8003_DDE8, K::Jr { rs: Reg::RA }),
        (
            0x8003_DDEC,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
    ];
    for (address, kind) in evidence {
        place(&mut text, address, kind);
    }
    for &(address, _, kind) in QUEST_ADDITIONAL_CONTROL_MEANING_EVIDENCE {
        place(&mut text, address, kind);
    }
    (map, text)
}

#[test]
fn 확정_의미_32종의_소비_evidence를_전부_요구한다() {
    let (map, text) = valid_fixture();
    assert_eq!(
        verify_box_quest_control_meanings(&map, &text),
        Ok(QuestControlMeaningSummary {
            confirmed_meanings: 32,
            advance_line_runtime: QUEST_ADVANCE_LINE_RUNTIME,
            close_text_window_runtime: QUEST_CLOSE_TEXT_WINDOW_RUNTIME,
            open_text_window_runtime: QUEST_OPEN_TEXT_WINDOW_RUNTIME,
            release_object_runtime: QUEST_RELEASE_OBJECT_RUNTIME,
            allocate_object_runtime: QUEST_ALLOCATE_OBJECT_RUNTIME,
        })
    );
}

#[test]
fn 한_명령_drift도_의미_확정으로_승인하지_않는다() {
    let (map, mut text) = valid_fixture();
    place(
        &mut text,
        0x8002_C6B0,
        K::Sh {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x0E,
        },
    );
    assert!(matches!(
        verify_box_quest_control_meanings(&map, &text),
        Err(QuestControlMeaningError::InstructionMismatch {
            address: 0x8002_C6B0,
            ..
        })
    ));
}

#[test]
fn quest가_아닌_exe와_잘린_text를_거부한다() {
    let (map, text) = valid_fixture();
    let mut wrong = map;
    wrong.name = "PUYO2.EXE";
    assert_eq!(
        verify_box_quest_control_meanings(&wrong, &text),
        Err(QuestControlMeaningError::WrongExecutable("PUYO2.EXE"))
    );
    assert_eq!(
        verify_box_quest_control_meanings(&map, &text[..4]),
        Err(QuestControlMeaningError::TextLengthMismatch {
            expected: TEST_TEXT_LEN,
            actual: 4,
        })
    );
}
