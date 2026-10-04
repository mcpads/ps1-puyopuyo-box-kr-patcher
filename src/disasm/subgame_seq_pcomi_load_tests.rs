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

fn nop() -> K {
    K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    }
}

#[test]
fn typed_instruction_불일치를_거부한다() {
    let map = map(1);
    let bytes = encode_all(&[nop()]);
    let instructions = linear::sweep(&map, &bytes).unwrap();
    let failure = require(
        &instructions,
        map.load_addr,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 1,
        },
    )
    .unwrap_err();
    assert!(failure.to_string().contains("명령 불일치"));
}

#[test]
fn source_record의_포인터_변경을_거부한다() {
    let map = ExeMap {
        name: "PCOMI.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x0004_9800,
        entry_pc: 0x8004_7978,
        file: LogicalFile::new(0, 0x0004_A000),
    };
    let mut text = vec![0; map.text_len as usize];
    let mut sequence = vec![0; PCOMI1_SEQUENCE_BYTES];
    let write = |text: &mut [u8], runtime: u32, bytes: &[u8]| {
        let offset = map.text_offset(runtime).unwrap();
        text[offset..offset + bytes.len()].copy_from_slice(bytes);
    };
    write(&mut text, PCOMI1_LOAD_RECORD_RUNTIME, &LOAD_RECORD);
    write(
        &mut text,
        PCOMI1_MASTER_CONTINUATION_RUNTIME,
        &MASTER_CONTINUATION,
    );
    let mut broken_call = MASTER_CALL;
    broken_call[4] ^= 1;
    write(&mut text, PCOMI1_MASTER_CALL_RUNTIME, &broken_call);
    sequence[PCOMI1_OVERLAY_ENTRY_OFFSET..PCOMI1_OVERLAY_ENTRY_OFFSET + OVERLAY_ENTRY.len()]
        .copy_from_slice(&OVERLAY_ENTRY);

    let failure = verify_source_records(&map, &text, &sequence).unwrap_err();
    assert!(failure.to_string().contains("master call"));
}

#[test]
fn dispatcher_slot은_exact_handler를_요구한다() {
    let map = ExeMap {
        name: "PCOMI.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x0004_9800,
        entry_pc: 0x8004_7978,
        file: LogicalFile::new(0, 0x0004_A000),
    };
    let mut text = vec![0; map.text_len as usize];
    for (opcode, handler) in [
        (0x03u8, JUMP_HANDLER_RUNTIME),
        (0x04, CALL_HANDLER_RUNTIME),
        (0x11, CONTINUATION_HANDLER_RUNTIME),
        (0x28, NESTED_COMMAND_HANDLER_RUNTIME),
        (0x62, DEFAULT_ADVANCE_HANDLER_RUNTIME),
    ] {
        let runtime = MASTER_DISPATCH_TABLE_RUNTIME + u32::from(opcode) * 4;
        let offset = map.text_offset(runtime).unwrap();
        text[offset..offset + 4].copy_from_slice(&handler.to_le_bytes());
    }
    let nested_slot = map
        .text_offset(NESTED_DISPATCH_TABLE_RUNTIME + 0x10 * 4)
        .unwrap();
    text[nested_slot..nested_slot + 4].copy_from_slice(&RESOURCE_SELECTOR_RUNTIME.to_le_bytes());
    assert!(verify_dispatch_slots(&map, &text).is_ok());

    let slot = map
        .text_offset(MASTER_DISPATCH_TABLE_RUNTIME + 0x62 * 4)
        .unwrap();
    text[slot] ^= 1;
    let failure = verify_dispatch_slots(&map, &text).unwrap_err();
    assert!(failure.to_string().contains("opcode=0x62"));
}

#[test]
fn overlay_entry는_load_base와_파일_offset을_섞지_않는다() {
    assert_eq!(
        PCOMI1_LOAD_BASE + PCOMI1_OVERLAY_ENTRY_OFFSET as u32,
        PCOMI1_OVERLAY_ENTRY_RUNTIME
    );
    assert_eq!(
        PCOMI1_LOAD_BASE + PCOMI1_OVERLAY_START_OFFSET as u32,
        PCOMI1_OVERLAY_START_RUNTIME
    );
    assert_ne!(PCOMI1_OVERLAY_ENTRY_RUNTIME, PCOMI1_MASTER_CALL_RUNTIME);
}
