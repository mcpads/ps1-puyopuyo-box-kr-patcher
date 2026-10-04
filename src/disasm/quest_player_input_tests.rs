use super::*;

#[test]
fn lui뒤_signed_offset을_부호확장해_입력주소를_계산한다() {
    assert_eq!(QUEST_PLAYER_HELD_INPUT_RUNTIME, 0x800F_D6B8);
    assert_eq!(QUEST_PLAYER_PRESSED_INPUT_RUNTIME, 0x800F_D6BC);
    assert_ne!(QUEST_PLAYER_HELD_INPUT_RUNTIME, 0x8010_D6B8);
}
