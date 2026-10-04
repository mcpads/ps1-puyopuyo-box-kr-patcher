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
fn pressed_input의_상호작용_bit와_방향별_low_contact를_결속한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_player_interaction_contact(map, &text),
        Ok(QuestPlayerInteractionContactSummary {
            input_runtime: 0x800F_D6BC,
            input_mask: 0x0020,
            writer_call_runtime: 0x8005_7510,
            writer_runtime: 0x8005_5BFC,
            collision_call_runtime: 0x8005_5CFC,
            collision_query_runtime: 0x8006_E550,
            direction_offset: 0xA5,
            fixed_position_offsets: [0xA8, 0xAC],
            rectangle_offsets: [0xB0, 0xB2, 0xB4, 0xB6],
            contact_flags_offset: 0xB8,
            direction_contact_masks: [1, 2, 4, 8],
            direction_vectors: [[0, 1], [-1, 0], [0, -1], [1, 0]],
            probe_gap_pixels: 4,
            probe_extent_pixels: 4,
            probe_cross_axis_inset_pixels: 1,
            probe_cross_axis_extent_trim_pixels: 2,
        })
    );
}

#[test]
fn 관측_player의_네_방향_probe를_source_수식대로_만든다() {
    let actor = QuestCollisionActorGeometry {
        position_fixed: [137 << 12, 596 << 12],
        local_rectangle: [-7, -13, 14, 15],
    };
    assert_eq!(
        quest_player_interaction_probe_rectangle(actor, 0).unwrap(),
        QuestCollisionRectangle {
            x: 131,
            y: 602,
            horizontal_extent: 12,
            vertical_extent: 4,
        }
    );
    assert_eq!(
        quest_player_interaction_probe_rectangle(actor, 1).unwrap(),
        QuestCollisionRectangle {
            x: 126,
            y: 584,
            horizontal_extent: 4,
            vertical_extent: 13,
        }
    );
    assert_eq!(
        quest_player_interaction_probe_rectangle(actor, 2).unwrap(),
        QuestCollisionRectangle {
            x: 131,
            y: 579,
            horizontal_extent: 12,
            vertical_extent: 4,
        }
    );
    assert_eq!(
        quest_player_interaction_probe_rectangle(actor, 3).unwrap(),
        QuestCollisionRectangle {
            x: 148,
            y: 584,
            horizontal_extent: 4,
            vertical_extent: 13,
        }
    );
}

#[test]
fn 입력_mask나_probe_방향이_바뀌면_source_gate가_거부한다() {
    let (map, mut text) = fixture();
    let input_offset = map.text_offset(0x8005_73D4).unwrap();
    text[input_offset..input_offset + 4].copy_from_slice(
        &encode(K::Andi {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 0x0080,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_player_interaction_contact(map, &text).is_err());

    let (map, mut text) = fixture();
    let direction_offset = map.text_offset(0x8005_5C38).unwrap();
    text[direction_offset..direction_offset + 4].copy_from_slice(
        &encode(K::Bne {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 25,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_player_interaction_contact(map, &text).is_err());
}

#[test]
fn 범위밖_방향과_너무_작은_actor_사각형을_거부한다() {
    assert!(quest_player_interaction_local_probe([-7, -13, 14, 15], 4).is_err());
    assert!(quest_player_interaction_local_probe([0, 0, 1, 1], 2).is_err());
}
