//! 설치된 OP21 훅 전용의 같은 SEQ 창 복귀 record. 원본 OP03 ABI와 구별한다.
use super::quest_translation_routing::QUEST_SEQ_ACTIVE_WINDOW_BYTES;
use crate::script::QUEST_SEQ_ACTIVE_RUNTIME_BASE;

pub(super) const QUEST_COMPACT_RETURN_BYTES: usize = 4;
pub(super) const QUEST_COMPACT_RETURN_MARKER: u8 = 0xFF;

pub(super) fn encode_return(target: u32) -> Result<[u8; QUEST_COMPACT_RETURN_BYTES], String> {
    let offset = target
        .checked_sub(QUEST_SEQ_ACTIVE_RUNTIME_BASE)
        .ok_or("QUEST 복귀 주소가 active base보다 작음")?;
    if offset == 0 || offset >= QUEST_SEQ_ACTIVE_WINDOW_BYTES as u32 || !offset.is_multiple_of(4) {
        return Err("QUEST 복귀 주소가 같은 active 창의 정렬된 비영 offset이 아님".into());
    }
    let word = u16::try_from(offset / 4).map_err(|_| "QUEST 복귀 offset 표현 범위 초과")?;
    let [lo, hi] = word.to_le_bytes();
    Ok([0x21, QUEST_COMPACT_RETURN_MARKER, lo, hi])
}

#[cfg(test)]
#[path = "quest_compact_return_tests.rs"]
mod tests;
