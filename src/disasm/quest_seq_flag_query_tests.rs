use super::*;

#[test]
fn flag_operand를_runtime_word와_mask로_해석한다() {
    let probe = resolve_quest_seq_flag_probe(0x8000, 0x11).expect("flag probe");

    assert_eq!(probe.word_index, 0x80);
    assert_eq!(probe.runtime_address, 0x800D_B8C0);
    assert_eq!(probe.bit_index, 17);
    assert_eq!(probe.mask, 0x0002_0000);
    assert_eq!(probe.queried_value(0), 0);
    assert_eq!(probe.queried_value(0x0002_0000), 1);
    assert_eq!(probe.queried_value(u32::MAX), 1);
}

#[test]
fn flag_base의_low_byte와_offset의_carry를_보존한다() {
    let probe = resolve_quest_seq_flag_probe(0x80FF, 1).expect("carry가 있는 flag probe");

    assert_eq!(probe.word_index, 0x88);
    assert_eq!(probe.runtime_address, 0x800D_B8E0);
    assert_eq!(probe.bit_index, 0);
    assert_eq!(probe.mask, 1);
}
