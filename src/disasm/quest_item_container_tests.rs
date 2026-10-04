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
fn op8e_item_container의_생성_개방_획득을_결속한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_item_container(map, &text),
        Ok(QuestItemContainerSummary {
            opcode: 0x8E,
            handler_runtime: 0x8007_41C8,
            callback_runtime: 0x8006_1A80,
            object_type: 4,
            record_bytes: 12,
            record_offsets: [1, 2, 4, 6, 7, 8],
            callback_offset: 0x18,
            layer_offset: 0x54,
            fixed_position_offsets: [0xA8, 0xAC],
            rectangle_offsets: [0xB0, 0xB2, 0xB4, 0xB6],
            local_rectangle: [-15, -15, 30, 15],
            collision_member_flags_offset: 0xBA,
            collision_member_mask: 2,
            contact_flags_offset: 0xB8,
            state_offset: 0xC6,
            persistence_field_offset: 0xA4,
            parameter_field_offset: 0x9C,
            item_id_field_offset: 0xCC,
            open_contact_mask: 4,
            attack_reaction_mask: 0x0F00,
            open_direction: 2,
            fixed_point_shift: 12,
            item_acquisition_state_machine_runtime: 0x8006_125C,
        })
    );
}

#[test]
fn 개방_contact_mask나_사각형이_바뀌면_거부한다() {
    let (map, mut text) = fixture();
    let mask_offset = map.text_offset(0x8006_1B00).unwrap();
    text[mask_offset..mask_offset + 4].copy_from_slice(
        &encode(K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 8,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_item_container(map, &text).is_err());

    let (map, mut text) = fixture();
    let rectangle_offset = map.text_offset(0x8007_42A4).unwrap();
    text[rectangle_offset..rectangle_offset + 4].copy_from_slice(
        &encode(K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 32,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_item_container(map, &text).is_err());
}

#[test]
fn op8e_dispatch가_다른_handler라면_모델과_불일치한다() {
    assert_eq!(quest_seq_opcode_handler(0x8E), Some(0x8007_41C8));
    assert_ne!(quest_seq_opcode_handler(0x8F), Some(0x8007_41C8));
}
