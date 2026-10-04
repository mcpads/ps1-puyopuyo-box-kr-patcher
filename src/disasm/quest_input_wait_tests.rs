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
    for &(address, _, kind) in QUEST_INPUT_WAIT_EVIDENCE {
        place(&mut text, address, kind);
    }
    (map, text)
}

#[test]
fn ff0f는_input_wait만_소유하고_window_transition을_직접_수행하지_않는다() {
    let (map, text) = valid_fixture();
    assert_eq!(
        verify_box_quest_input_wait(&map, &text),
        Ok(QuestInputWaitSummary {
            handler_runtime: QUEST_INPUT_WAIT_HANDLER_RUNTIME,
            start_runtime: QUEST_INPUT_WAIT_START_RUNTIME,
            poll_runtime: QUEST_INPUT_WAIT_POLL_RUNTIME,
            stream_bytes: 4,
            initial_count: 8,
            button_mask: 0x00F5,
            window_state_writes: 5,
            cursor_or_window_activation_writes: 0,
            known_window_transition_calls: 0,
        })
    );
}

#[test]
fn ff0f_helper의_cursor_write를_거부한다() {
    let (map, mut text) = valid_fixture();
    place(
        &mut text,
        0x8002_CC58,
        K::Sh {
            rt: Reg::ZERO,
            base: Reg::S0,
            offset: 0x0A,
        },
    );
    assert_eq!(
        verify_box_quest_input_wait(&map, &text),
        Err(QuestInputWaitError::UnexpectedWindowStateWrite {
            address: 0x8002_CC58,
            offset: 0x0A,
        })
    );
}

#[test]
fn ff0f_helper의_미등록_call을_거부한다() {
    let (map, mut text) = valid_fixture();
    place(
        &mut text,
        0x8002_CC74,
        K::Jal {
            target: QUEST_ADVANCE_LINE_RUNTIME,
        },
    );
    assert_eq!(
        verify_box_quest_input_wait(&map, &text),
        Err(QuestInputWaitError::UnexpectedHelperCall {
            address: 0x8002_CC74,
            target: QUEST_ADVANCE_LINE_RUNTIME,
        })
    );
}

#[test]
fn ff0f_input_mask_drift를_거부한다() {
    let (map, mut text) = valid_fixture();
    place(
        &mut text,
        0x8002_CD40,
        K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x00F4,
        },
    );
    assert!(matches!(
        verify_box_quest_input_wait(&map, &text),
        Err(QuestInputWaitError::InstructionMismatch {
            address: 0x8002_CD40,
            ..
        })
    ));
}
