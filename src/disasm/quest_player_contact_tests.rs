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
fn 플레이어_방향을_충돌_상대의_high_bit로_기록한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_player_contact(map, &text),
        Ok(QuestPlayerContactSummary {
            update_callback_runtime: 0x8005_5F44,
            contact_writer_call_runtime: 0x8005_7528,
            contact_writer_runtime: 0x8005_5DA0,
            collision_query_runtime: 0x8006_E550,
            direction_offset: 0xA5,
            contact_flags_offset: 0xB8,
            direction_contact_masks: [0x1000, 0x2000, 0x4000, 0x8000],
        })
    );
}

#[test]
fn 플레이어_callback이_contact_writer를_호출하지_않으면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map
        .text_offset(QUEST_PLAYER_CONTACT_WRITER_CALL_RUNTIME)
        .expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::J {
            target: QUEST_PLAYER_CONTACT_WRITER_RUNTIME,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_player_contact(map, &text).is_err());
}

#[test]
fn 방향과_high_bit의_대응이_바뀌면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8005_5F08).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Ori {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x4000,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_player_contact(map, &text).is_err());
}
