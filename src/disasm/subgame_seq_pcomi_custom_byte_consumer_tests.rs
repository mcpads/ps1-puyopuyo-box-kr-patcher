use crate::coords::{ExeMap, LogicalFile};
use crate::isa::{encode_all, InstructionKind as K};

use super::*;

fn map(words: usize) -> ExeMap {
    ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len: (words * 4) as u32,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, (words * 4 + 0x800) as u32),
    }
}

fn write_instruction(map: &ExeMap, text: &mut [u8], runtime: u32, kind: K) {
    let offset = map.text_offset(runtime).unwrap();
    text[offset..offset + 4].copy_from_slice(&encode_all(&[kind]));
}

#[test]
fn typed_instruction_불일치를_거부한다() {
    let map = map(1);
    let bytes = encode_all(&[K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    }]);
    let instructions = linear::sweep(&map, &bytes).unwrap();
    let failure = require(
        &instructions,
        map.load_addr,
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0,
        },
    )
    .unwrap_err();
    assert!(failure.to_string().contains("명령 불일치"));
}

#[test]
fn root_jump_table의_selector_순서를_고정한다() {
    assert_eq!(ROOT_CASES.len(), 8);
    assert_eq!(ROOT_CASES[0], (0x8002_03FC, 0x801A_6280));
    assert_eq!(
        ROOT_CASES[usize::from(PCOMI_CUSTOM_BYTE_SLOT_SELECTOR)],
        (0x8002_046C, PCOMI_CUSTOM_BYTE_ROOT_FIELD_RUNTIME)
    );
}

#[test]
fn slot_random_index가_전체_11개를_포괄한다() {
    assert_eq!(PCOMI_CUSTOM_BYTE_SLOT_COUNT, 11);
    assert_eq!(PCOMI_CUSTOM_BYTE_SLOT_PAYLOAD_BYTES, 216);
    assert_eq!(PCOMI_CUSTOM_BYTE_SLOT_SELECTOR, 7);
}

#[test]
fn admission_source_gate는_instruction과_action_table_drift를_거부한다() {
    let map = map(0x13_058 / 4);
    let mut text = vec![0; map.text_len as usize];
    for (runtime, kind) in slot_admission_checks() {
        write_instruction(&map, &mut text, runtime, kind);
    }
    let action_slot = PCOMI_CUSTOM_BYTE_SLOT_ACTION_TABLE_RUNTIME
        + u32::from(PCOMI_CUSTOM_BYTE_SLOT_ACTION_ROW) * 4;
    let action_offset = map.text_offset(action_slot).unwrap();
    text[action_offset..action_offset + 4].copy_from_slice(&0x8002_28E0_u32.to_le_bytes());
    let instructions = linear::sweep(&map, &text).unwrap();
    verify_slot_admission(&map, &text, &instructions).unwrap();

    write_instruction(
        &map,
        &mut text,
        PCOMI_CUSTOM_BYTE_SLOT_CALLBACK_RUNTIME,
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -0x38,
        },
    );
    let instructions = linear::sweep(&map, &text).unwrap();
    assert!(verify_slot_admission(&map, &text, &instructions)
        .unwrap_err()
        .to_string()
        .contains("명령 불일치"));

    write_instruction(
        &map,
        &mut text,
        PCOMI_CUSTOM_BYTE_SLOT_CALLBACK_RUNTIME,
        slot_admission_checks()[0].1,
    );
    text[action_offset..action_offset + 4].copy_from_slice(&0x8002_28E4_u32.to_le_bytes());
    let instructions = linear::sweep(&map, &text).unwrap();
    assert!(verify_slot_admission(&map, &text, &instructions)
        .unwrap_err()
        .to_string()
        .contains("action row dispatch 불일치"));
}

#[test]
fn custom_byte는_generic_0x96_offset없이_direct_cell로_전달된다() {
    assert_eq!(PCOMI_CUSTOM_BYTE_ORDINARY_TOKEN_LOAD_RUNTIME, 0x8002_8F68);
    assert_eq!(PCOMI_CUSTOM_BYTE_TOKEN_NODE_STORE_RUNTIME, 0x8002_9040);
    assert_eq!(PCOMI_CUSTOM_BYTE_TOKEN_NODE_LOAD_RUNTIME, 0x8002_948C);
    assert_eq!(PCOMI_CUSTOM_BYTE_DIRECT_CELL_STORE_RUNTIME, 0x8002_9498);
}
