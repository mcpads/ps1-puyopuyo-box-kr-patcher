use super::*;
use crate::coords::exe_map;
use crate::isa::{encode, Reg};

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    for (address, _, kind) in required_instructions() {
        let offset = map.text_offset(address).expect("fixture 명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
    }
    for (address, _, word) in required_data_words() {
        let offset = map.text_offset(address).expect("fixture data 범위");
        text[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }
    (map, text)
}

#[test]
fn 세번째_qmp_plane을_publish해_플레이어_이동에서_소비한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_map_collision(map, &text),
        Ok(QuestMapCollisionSummary {
            active_slot_runtime: 0x800B_E5A4,
            slot_buffer_roots_runtime: 0x800B_E5A8,
            slot_widths_runtime: 0x800B_E5E0,
            slot_heights_runtime: 0x800B_E5E8,
            collision_buffer_runtime: 0x800F_7488,
            slot_count: 2,
            plane_capacity_cells: 0x2400,
            plane_stride_bytes: 0x4800,
            collision_plane_offset: 0x9000,
            plane_loader_runtimes: [0x8007_6E50, 0x8007_6F04, 0x8007_6F98],
            qmp_header_bytes: 20,
            classifier_runtime: 0x8006_B928,
            movement_runtime: 0x8006_BB9C,
            player_movement_call_runtime: 0x8005_7EA0,
            player_object_movement_call_runtime: 0x8005_7EC4,
            player_collision_cancel_runtime: 0x8005_8000,
            player_position_commit_runtime: 0x8005_81C0,
            player_normal_movement_state: 0,
            player_normal_movement_input_runtime: 0x800F_D6B8,
            player_normal_movement_input_masks: [0x4000, 0x8000, 0x1000, 0x2000],
            player_normal_movement_deltas_fixed: [
                [0, 0x2000],
                [-0x2000, 0],
                [0, -0x2000],
                [0x2000, 0],
            ],
            player_normal_movement_step_pixels: 2,
            rectangle_overlap_runtime: 0x8006_B61C,
            movement_adjust_runtime: 0x8006_B7BC,
            actor_fixed_position_offsets: [0xA8, 0xAC],
            actor_rectangle_offsets: [0xB0, 0xB2, 0xB4, 0xB6],
            actor_member_flags_offset: 0xBA,
            actor_map_member_mask: 0x0001,
            fixed_point_shift: 12,
            tile_size_pixels: 16,
            tile_interior_inset_pixels: 1,
            tile_interior_extent_pixels: 14,
            probe_extent_margin_pixels: 4,
            map_bounds_runtimes: [0x800B_E5C8, 0x800B_E5CA, 0x800B_E5CC, 0x800B_E5CE],
            tile_low_byte_mask: 0x00FF,
            tile_flag_masks: [0x4000, 0x8000],
            explicit_low_bytes: [1, 2, 6, 97, 99, 102, 109, 110],
            explicit_range_start: 112,
            explicit_range_end: 127,
            none_class: 0,
            special_class: 1,
            solid_class: 2,
            actor_layer_offset: 0x54,
            actor_direction_offset: 0xA5,
            actor_state_offset: 0xC6,
            primary_actor_runtime: 0x800B_E600,
            condition_opcode: 0x92,
            condition_writer_runtime: 0x8007_8614,
            condition_runtimes: [0x800B_E5FC, 0x800B_E5FD],
        })
    );
}

#[test]
fn player_rectangle_pointer가_actor_b0이_아니면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8005_7E84).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Addiu {
            rt: Reg::S3,
            rs: Reg::S4,
            imm: 0xA8,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}

#[test]
fn tile_interior_extent가_14가_아니면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8006_BE3C).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 16,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}

#[test]
fn movement_adjust가_overlap_width를_읽지_않으면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8006_B8B8).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Lh {
            rt: Reg::A1,
            base: Reg::T2,
            offset: 6,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}

#[test]
fn 정상_보행_delta_scale이_2px가_아니면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8005_7648).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Sll {
            rd: Reg::V1,
            rt: Reg::S6,
            sa: 12,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}

#[test]
fn 정상_state의_이동_handler_pointer가_달라지면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8001_2688).expect("fixture data 범위");
    text[offset..offset + 4].copy_from_slice(&0x8005_7E40u32.to_le_bytes());
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}

#[test]
fn 충돌뒤_delta_cancel이_사라지면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8005_800C).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Sw {
            rt: Reg::ZERO,
            base: Reg::SP,
            offset: 0xD8,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}

#[test]
fn 충돌_plane_offset이_달라지면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8007_6FE4).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Ori {
            rt: Reg::A3,
            rs: Reg::ZERO,
            imm: 0x4800,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}

#[test]
fn 두번째_qmp_plane_stride가_달라지면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8007_6F6C).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 0x4000,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}

#[test]
fn 세번째_map_descriptor의_callback이_충돌_plane_loader가_아니면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8007_12C0).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Addiu {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: 0x6F04,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}

#[test]
fn classifier의_low_byte_mask가_달라지면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8006_B92C).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Andi {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 0x03FF,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}

#[test]
fn op92_set조건의_branch방향이_바뀌면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8006_BA84).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 67,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}

#[test]
fn 인접_tile의_solid_class_경계가_달라지면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8006_BFD4).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 3,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}

#[test]
fn op92_조건_writer가_두번째_byte를_쓰지_않으면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8007_8638).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Sb {
            rt: Reg::V0,
            base: Reg::AT,
            offset: -0x1A04,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_map_collision(map, &text).is_err());
}
