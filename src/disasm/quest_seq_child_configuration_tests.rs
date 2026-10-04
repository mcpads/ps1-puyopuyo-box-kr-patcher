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
    for (index, target) in JUMP_TABLE.into_iter().enumerate() {
        let address = QUEST_SEQ_CHILD_CONFIGURATION_TABLE_RUNTIME + index as u32 * 4;
        let offset = map.text_offset(address).expect("fixture table 범위");
        text[offset..offset + 4].copy_from_slice(&target.to_le_bytes());
    }
    (map, text)
}

#[test]
fn child의_위치_접촉영역_상대이동과_12바이트_전진을_검증한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_seq_child_configuration(map, &text),
        Ok(QuestSeqChildConfigurationSummary {
            opcode: 0x83,
            handler_runtime: 0x8007_33A4,
            subtype_limit: 6,
            table_runtime: 0x8001_3038,
            record_bytes: 12,
            child_pointer_field_offset: 0x34,
            position_arm_runtime: 0x8007_33F0,
            bounds_arm_runtime: 0x8007_34B8,
            motion_arm_runtime: 0x8007_3508,
            fixed_point_shift: 12,
        })
    );
}

#[test]
fn 위치_환산이나_child_store가_drift하면_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8007_3408).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 8,
        })
        .to_le_bytes(),
    );
    assert!(verify_box_quest_seq_child_configuration(map, &text).is_err());
}

#[test]
fn subtype_table_arm이_바뀌면_거부한다() {
    let (map, mut text) = fixture();
    let address = QUEST_SEQ_CHILD_CONFIGURATION_TABLE_RUNTIME + 2 * 4;
    let offset = map.text_offset(address).expect("fixture table 범위");
    text[offset..offset + 4].copy_from_slice(&0x8007_3508u32.to_le_bytes());
    assert!(verify_box_quest_seq_child_configuration(map, &text).is_err());
}
