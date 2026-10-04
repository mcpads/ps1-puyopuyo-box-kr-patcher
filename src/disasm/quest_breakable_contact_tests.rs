use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    for (address, _, kind) in required_instructions() {
        let offset = map.text_offset(address).expect("fixture 명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
    }
    (map, text)
}

#[test]
fn 공격_contact를_소비해_상태를_옮기고_객체를_해제한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_breakable_contact(map, &text),
        Ok(QuestBreakableContactSummary {
            update_callback_runtime: 0x8006_3820,
            state_offset: 0xC6,
            contact_flags_offset: 0xB8,
            attack_contact_mask: 0x0F00,
            release_call_runtime: 0x8006_3B38,
            release_object_runtime: 0x8001_4CD0,
        })
    );
}

#[test]
fn 공격_contact_mask가_바뀌면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8006_3864).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0xF000,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_breakable_contact(map, &text).is_err());
}

#[test]
fn 객체_해제_호출이_바뀌면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map
        .text_offset(QUEST_BREAKABLE_RELEASE_CALL_RUNTIME)
        .expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::J {
            target: QUEST_OBJECT_RELEASE_RUNTIME,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_breakable_contact(map, &text).is_err());
}
