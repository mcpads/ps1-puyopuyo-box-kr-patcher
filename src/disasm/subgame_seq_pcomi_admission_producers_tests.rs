use crate::coords::{ExeMap, LogicalFile};

use super::*;

fn map(text_len: u32) -> ExeMap {
    ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, text_len + 0x800),
    }
}

fn write_word(map: &ExeMap, text: &mut [u8], runtime: u32, value: u32) {
    let offset = map.text_offset(runtime).unwrap();
    text[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn callback_index가_같아도_self_loop가_아니면_producer로_세지_않는다() {
    let base = 0x801A_0000u32;
    let mut raw = vec![0u8; 0x80];
    raw[0x10..0x20].copy_from_slice(&REJECTED_HALFWORD_BYTES);
    raw[0x40..0x4C].copy_from_slice(&[
        0x62, 0x00, 0x1D, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
    ]);
    raw[0x4C..0x50].copy_from_slice(&(base + 0x40).to_le_bytes());

    assert_eq!(
        self_repeating_callbacks(&raw, base),
        vec![SelfRepeatingCallback {
            offset: 0x40,
            callback_index: PCOMI_ADMISSION_CALLBACK_INDEX,
        }]
    );
}

#[test]
fn callback_self_pointer_drift를_거부한다() {
    let base = 0x801A_0000u32;
    let mut raw = vec![0u8; 0x40];
    raw[0x10..0x1C].copy_from_slice(&[
        0x62, 0x00, 0x1D, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
    ]);
    raw[0x1C..0x20].copy_from_slice(&(base + 0x14).to_le_bytes());
    assert!(self_repeating_callbacks(&raw, base).is_empty());
}

#[test]
fn opcode_0x62에_다른_local_handler를_설치하면_거부한다() {
    let map = map(0x41400);
    let mut text = vec![0u8; map.text_len as usize];
    write_word(
        &map,
        &mut text,
        PCOMI_MAIN_OPCODE_TABLE_RUNTIME + u32::from(PCOMI_CALLBACK_DISPATCH_OPCODE) * 4,
        PCOMI_DEFAULT_ADVANCE_RUNTIME,
    );
    for index in 0..PCOMI_LOCAL_HANDLER_COUNT {
        let value = expected_local_handler_runtime(index);
        write_word(
            &map,
            &mut text,
            PCOMI_LOCAL_HANDLER_SOURCE_RUNTIME + index as u32 * 4,
            value,
        );
    }
    for (index, value) in [
        (PCOMI_UI_CALLBACK_INDEX, PCOMI_UI_CALLBACK_RUNTIME),
        (
            PCOMI_ADMISSION_CALLBACK_INDEX,
            PCOMI_ADMISSION_CALLBACK_RUNTIME,
        ),
    ] {
        write_word(
            &map,
            &mut text,
            PCOMI_LOCAL_CALLBACK_TABLE_RUNTIME + u32::from(index) * 4,
            value,
        );
    }
    verify_handler_source(&map, &text).unwrap();

    write_word(
        &map,
        &mut text,
        PCOMI_LOCAL_HANDLER_SOURCE_RUNTIME + 2 * 4,
        0x8002_9688,
    );
    assert!(verify_handler_source(&map, &text)
        .unwrap_err()
        .to_string()
        .contains("local handler source 불일치"));
}

#[test]
fn admission_callback_table_slot_drift를_거부한다() {
    let map = map(0x41400);
    let mut text = vec![0u8; map.text_len as usize];
    write_word(
        &map,
        &mut text,
        PCOMI_MAIN_OPCODE_TABLE_RUNTIME + u32::from(PCOMI_CALLBACK_DISPATCH_OPCODE) * 4,
        PCOMI_DEFAULT_ADVANCE_RUNTIME,
    );
    for index in 0..PCOMI_LOCAL_HANDLER_COUNT {
        let value = expected_local_handler_runtime(index);
        write_word(
            &map,
            &mut text,
            PCOMI_LOCAL_HANDLER_SOURCE_RUNTIME + index as u32 * 4,
            value,
        );
    }
    write_word(
        &map,
        &mut text,
        PCOMI_LOCAL_CALLBACK_TABLE_RUNTIME + u32::from(PCOMI_UI_CALLBACK_INDEX) * 4,
        PCOMI_UI_CALLBACK_RUNTIME,
    );
    write_word(
        &map,
        &mut text,
        PCOMI_LOCAL_CALLBACK_TABLE_RUNTIME + u32::from(PCOMI_ADMISSION_CALLBACK_INDEX) * 4,
        PCOMI_ADMISSION_CALLBACK_RUNTIME + 4,
    );
    assert!(verify_handler_source(&map, &text)
        .unwrap_err()
        .to_string()
        .contains("local callback table 불일치"));
}
