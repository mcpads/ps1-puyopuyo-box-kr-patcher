use crate::coords::LogicalFile;
use crate::isa::{encode_all, InstructionKind as K, Reg};

use super::*;

fn test_map(text_len: u32) -> ExeMap {
    ExeMap {
        name: "QUEST.EXE",
        load_addr: 0x8001_0000,
        text_len,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, text_len + 0x800),
    }
}

#[test]
fn 후보는_zero_clear뒤_sector경계부터_첫정적참조직전이다() {
    assert_eq!(QUEST_PAGE_CARRIER_START_RUNTIME, 0x8010_8800);
    assert_eq!(QUEST_PAGE_CARRIER_END_RUNTIME, 0x8011_9008);
    assert_eq!(QUEST_PAGE_CARRIER_SEARCH_END_RUNTIME, 0x8012_1724);
    assert_eq!(QUEST_PAGE_CARRIER_CAPACITY_BYTES, 67_592);
    assert_eq!(
        QUEST_PAGE_CARRIER_START_RUNTIME - QUEST_HOOK_ZERO_CLEAR_END_RUNTIME,
        1_208
    );
}

#[test]
fn 주소조립과_raw_pointer를_서로_독립적으로_센다() {
    let map = test_map(16);
    let mut text = encode_all(&[
        K::Lui {
            rt: Reg::T0,
            imm: 0x8011,
        },
        K::Ori {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 0x1234,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    text.extend_from_slice(&0x8011_2344u32.to_le_bytes());
    assert_eq!(
        count_static_references(
            &map,
            &text,
            QUEST_PAGE_CARRIER_START_RUNTIME,
            QUEST_PAGE_CARRIER_END_RUNTIME,
        )
        .unwrap(),
        (1, 1, Some(0x8011_1234))
    );
}

#[test]
fn 후보밖_주소는_참조로_세지_않는다() {
    let map = test_map(12);
    let text = encode_all(&[
        K::Lui {
            rt: Reg::T0,
            imm: 0x8012,
        },
        K::Ori {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 0x1724,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    assert_eq!(
        count_static_references(
            &map,
            &text,
            QUEST_PAGE_CARRIER_START_RUNTIME,
            QUEST_PAGE_CARRIER_END_RUNTIME,
        )
        .unwrap(),
        (0, 0, None)
    );
}
