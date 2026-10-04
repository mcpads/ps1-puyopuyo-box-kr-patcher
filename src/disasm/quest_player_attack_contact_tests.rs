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
fn 공격_입력과_방향을_상대의_middle_contact_bit로_기록한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_player_attack_contact(map, &text),
        Ok(QuestPlayerAttackContactSummary {
            input_runtime: 0x800F_D6B8,
            input_mask: 0x0040,
            attack_state_offset: 0xC0,
            attack_state: 2,
            direction_offset: 0xA5,
            collision_call_runtime: 0x8005_672C,
            collision_query_runtime: 0x8006_E550,
            target_contact_flags_offset: 0xB8,
            direction_contact_masks: [0x0100, 0x0200, 0x0400, 0x0800],
        })
    );
}

#[test]
fn 입력_mask가_바뀌면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8005_65BC).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x0020,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_player_attack_contact(map, &text).is_err());
}

#[test]
fn 방향과_middle_contact_bit의_대응이_바뀌면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8005_6794).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Ori {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x0400,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_player_attack_contact(map, &text).is_err());
}
