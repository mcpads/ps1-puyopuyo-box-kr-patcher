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
fn op82_child는_접촉_flag로_state1이_된_뒤_interpreter를_호출한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_seq_child_activation(map, &text),
        Ok(QuestSeqChildActivationSummary {
            subtype2_arm_runtime: 0x8007_29E0,
            update_callback_runtime: 0x8005_8C80,
            state_offset: 0xC6,
            contact_flags_offset: 0xB8,
            direct_contact_mask: 0x000F,
            alternate_contact_mask: 0xF000,
            collision_member_flags_offset: 0xBA,
            collision_member_mask: 0x0002,
            collision_layer_offset: 0x54,
            collision_query_runtime: 0x8006_E550,
            contact_writer_runtime: 0x8005_4F8C,
            interpreter_runtime: 0x8002_2570,
        })
    );
}

#[test]
fn 접촉_mask가_바뀌면_활성화_경로를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8005_8D1C).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Andi {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 0x0003,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_seq_child_activation(map, &text).is_err());
}

#[test]
fn 충돌_검색이_다른_layer를_허용하면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8006_E60C).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 42,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_seq_child_activation(map, &text).is_err());
}

#[test]
fn 방향과_contact_bit의_대응이_바뀌면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8005_4FA8).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Bne {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 13,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_seq_child_activation(map, &text).is_err());
}
