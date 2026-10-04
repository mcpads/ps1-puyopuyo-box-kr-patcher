use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn instruction(addr: u32, kind: K) -> Instruction {
    Instruction {
        addr,
        word: 0,
        kind,
    }
}

fn selector_fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    for (index, handler) in QUEST_RESIDENT_ROOT_SELECTOR_HANDLERS.iter().enumerate() {
        let runtime = QUEST_RESIDENT_ROOT_DISPATCH_TABLE_RUNTIME
            + u32::try_from(index).expect("fixture selector index") * 4;
        let offset = map.text_offset(runtime).expect("fixture table 범위");
        text[offset..offset + 4].copy_from_slice(&handler.to_le_bytes());
    }
    for (runtime, kind) in [
        (
            0x8007_3DF4,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8010,
            },
        ),
        (
            0x8007_3DF8,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_RESIDENT_ROOT_STATE_BASE_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8007_3DFC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_RESIDENT_ROOT_STATE_BASE_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8007_3E04,
            K::Beq {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 25,
            },
        ),
        (
            0x8007_3E6C,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8007_3E74,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 8,
            },
        ),
        (
            0x8007_3E78,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 197,
            },
        ),
        (
            0x8007_3E7C,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8001,
            },
        ),
        (
            0x8007_3E80,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x3050,
            },
        ),
        (
            0x8007_3E84,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            0x8007_3E88,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8007_3E8C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (0x8007_3E94, K::Jr { rs: Reg::V0 }),
        (
            0x8007_3F0C,
            K::Sb {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: QUEST_RESIDENT_ROOT_SELECTOR_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8007_3FB0,
            K::Sb {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: QUEST_RESIDENT_ROOT_SELECTOR_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8007_3FA4,
            K::J {
                target: 0x8007_404C,
            },
        ),
    ] {
        let offset = map.text_offset(runtime).expect("fixture 명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
    }
    (map, text)
}

fn verify_selector_fixture(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestResidentSeqRootSelection, QuestResidentSeqRootError> {
    let instructions = decode_slice(map.load_addr, text).expect("fixture 정렬");
    verify_root_selection(map, text, &instructions)
}

#[test]
fn selector_4와_5를_작별과_패배_constructor에_결속한다() {
    let (map, text) = selector_fixture();

    let selection = verify_selector_fixture(map, &text).expect("selector 검증");

    assert_eq!(selection.selector_runtime, 0x800F_BDD4);
    assert_eq!(selection.farewell_selector, 4);
    assert_eq!(selection.farewell_handler_runtime, 0x8007_3F08);
    assert_eq!(selection.farewell_root_runtime, 0x8008_97DC);
    assert_eq!(selection.defeat_selector, 5);
    assert_eq!(selection.defeat_handler_runtime, 0x8007_3FAC);
    assert_eq!(selection.defeat_root_runtime, 0x8008_C498);
}

#[test]
fn selector_table의_target_drift를_거부한다() {
    let (map, mut text) = selector_fixture();
    let defeat_entry_runtime =
        QUEST_RESIDENT_ROOT_DISPATCH_TABLE_RUNTIME + u32::from(QUEST_RESIDENT_DEFEAT_SELECTOR) * 4;
    let offset = map
        .text_offset(defeat_entry_runtime)
        .expect("defeat table entry 범위");
    text[offset..offset + 4].copy_from_slice(&0x8007_3F08u32.to_le_bytes());

    let failure = verify_selector_fixture(map, &text).expect_err("table drift 거부");

    assert!(failure.to_string().contains("selector table drift"));
}

#[test]
fn selector_state_field_drift를_거부한다() {
    let (map, mut text) = selector_fixture();
    let offset = map.text_offset(0x8007_3E6C).expect("selector load 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Lbu {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 5,
        })
        .to_le_bytes(),
    );

    let failure = verify_selector_fixture(map, &text).expect_err("field drift 거부");

    assert!(failure.to_string().contains("selector load drift"));
}

#[test]
fn allocator와_다음_call사이의_stream_store만_센다() {
    let instructions = [
        instruction(
            0x1000,
            K::Jal {
                target: QUEST_OBJECT_ALLOCATE_RUNTIME,
            },
        ),
        instruction(0x1004, K::Unknown { word: 0 }),
        instruction(
            0x1008,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_SEQ_STREAM_FIELD_OFFSET,
            },
        ),
        instruction(
            0x100C,
            K::Jal {
                target: QUEST_OBJECT_ALLOCATE_RUNTIME,
            },
        ),
        instruction(
            0x1010,
            K::Jal {
                target: 0x8001_0000,
            },
        ),
        instruction(
            0x1014,
            K::Sw {
                rt: Reg::A1,
                base: Reg::V0,
                offset: QUEST_SEQ_STREAM_FIELD_OFFSET,
            },
        ),
    ];

    assert_eq!(constructor_stream_stores(&instructions), [(0x1000, 0x1008)]);
}

#[test]
fn 다른_allocator와_field는_resident_root로_세지_않는다() {
    let instructions = [
        instruction(
            0x2000,
            K::Jal {
                target: 0x8001_0000,
            },
        ),
        instruction(
            0x2004,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_SEQ_STREAM_FIELD_OFFSET,
            },
        ),
        instruction(
            0x2008,
            K::Jal {
                target: QUEST_OBJECT_ALLOCATE_RUNTIME,
            },
        ),
        instruction(
            0x200C,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x20,
            },
        ),
    ];

    assert!(constructor_stream_stores(&instructions).is_empty());
}

#[test]
fn current_record_pointer_constructor는_op1c_child를_별도_object로_설치한다() {
    let handler =
        quest_seq_opcode_handler(QUEST_RESIDENT_CURRENT_RECORD_OPCODE).expect("OP1C handler");
    let next_handler =
        quest_seq_opcode_handler(QUEST_RESIDENT_CURRENT_RECORD_OPCODE + 1).expect("OP1D handler");
    let install = ROOT_INSTALL_SPECS
        .iter()
        .find(|spec| matches!(spec.source, RootSourceSpec::CurrentRecordPointer { .. }))
        .expect("current-record constructor");

    assert_eq!(handler, 0x8002_2C7C);
    assert!(handler <= install.allocator_call_runtime);
    assert!(install.stream_store_runtime < next_handler);
}
