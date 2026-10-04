//! QUEST 플레이어 callback이 읽는 입력 word 주소.
//!
//! R3000A의 `lui` 뒤 signed 16-bit 변위를 쓰는 load/addiu는 하위 값을
//! 단순 결합하지 않는다. 변위를 부호 확장해 더한 유효 주소를 세 입력 소비
//! 경로가 공유한다.

pub(super) const QUEST_PLAYER_INPUT_LUI_IMMEDIATE: u16 = 0x8010;
pub(super) const QUEST_PLAYER_HELD_INPUT_SIGNED_OFFSET: i16 = -10_568;
pub(super) const QUEST_PLAYER_PRESSED_INPUT_FIELD_OFFSET: u32 = 4;

pub(super) const QUEST_PLAYER_HELD_INPUT_RUNTIME: u32 = mips_lui_signed_offset_address(
    QUEST_PLAYER_INPUT_LUI_IMMEDIATE,
    QUEST_PLAYER_HELD_INPUT_SIGNED_OFFSET,
);
pub(super) const QUEST_PLAYER_PRESSED_INPUT_RUNTIME: u32 =
    QUEST_PLAYER_HELD_INPUT_RUNTIME + QUEST_PLAYER_PRESSED_INPUT_FIELD_OFFSET;

const fn mips_lui_signed_offset_address(lui_immediate: u16, signed_offset: i16) -> u32 {
    ((lui_immediate as u32) << 16).wrapping_add(signed_offset as i32 as u32)
}

#[cfg(test)]
#[path = "quest_player_input_tests.rs"]
mod tests;
