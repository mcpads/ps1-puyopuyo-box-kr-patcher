use super::*;

const SUKE_CONTACT_RECORDS: [u8; QUEST_SEQ_CHILD_CONTACT_RECORDS_BYTES] = [
    0x83, 0x00, 0x0E, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, // 절대 위치
    0x83, 0x03, 0x00, 0x00, 0xF8, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 상대 이동
    0x83, 0x02, 0xF0, 0xFF, 0xF0, 0xFF, 0x20, 0x00, 0x20, 0x00, 0x00, 0x00, // 접촉 영역
];

#[test]
fn suke_child의_접촉_영역을_세_op83_record에서_계산한다() {
    assert_eq!(
        parse_quest_seq_child_contact(&SUKE_CONTACT_RECORDS),
        Ok(QuestSeqChildContact {
            anchor_x_fixed: 240 << 12,
            anchor_y_fixed: 328 << 12,
            anchor_x_pixels: 240,
            anchor_y_pixels: 328,
            layer: 1,
            velocity_x_fixed: 0,
            velocity_y_fixed: 0,
            bounds_left: -16,
            bounds_top: -16,
            bounds_width: 32,
            bounds_height: 32,
            contact_left_pixels: 224,
            contact_top_pixels: 312,
            contact_right_pixels: 256,
            contact_bottom_pixels: 344,
        })
    );
}

#[test]
fn 세_record의_역할_순서가_바뀌면_거부한다() {
    let mut raw = SUKE_CONTACT_RECORDS;
    raw[1] = CONTACT_BOUNDS_SUBTYPE;
    assert!(parse_quest_seq_child_contact(&raw).is_err());
}

#[test]
fn 부분_record나_추가_byte를_정상_입력으로_받지_않는다() {
    assert!(parse_quest_seq_child_contact(&SUKE_CONTACT_RECORDS[..35]).is_err());
    let mut extended = SUKE_CONTACT_RECORDS.to_vec();
    extended.push(0);
    assert!(parse_quest_seq_child_contact(&extended).is_err());
}
