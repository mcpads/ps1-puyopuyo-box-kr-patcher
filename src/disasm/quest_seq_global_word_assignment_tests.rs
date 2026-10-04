use super::*;

#[test]
fn op11_40은_즉시값과_전역_word_주소를_가역적으로_드러낸다() {
    let raw = [0x11, 0x40, 0x38, 0xA5, 0x60, 0x5C, 0x08, 0x80];

    let assignment = parse_quest_seq_global_word_assignment(&raw).expect("OP11/0x40 record");

    assert_eq!(assignment.raw, raw);
    assert_eq!(assignment.state_index, 0x38);
    assert_eq!(assignment.unconsumed_parameter, 0xA5);
    assert_eq!(assignment.value, 0x8008_5C60);
    assert_eq!(assignment.state_runtime(), Some(0x800D_B7A0));
}

#[test]
fn 다른_op11_mode와_잘린_record는_대입으로_오인하지_않는다() {
    assert!(parse_quest_seq_global_word_assignment(&[0x11, 0x00, 0x38, 0, 1, 0, 0, 0]).is_none());
    assert!(parse_quest_seq_global_word_assignment(&[0x11, 0x40, 0x38, 0]).is_none());
}
