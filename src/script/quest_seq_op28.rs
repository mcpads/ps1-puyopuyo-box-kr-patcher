//! 공용 SEQ OP28의 32-entry 하위 dispatch 사양.
//!
//! OP28은 record `+1` byte를 별도 table에 넣는다. table 자체에는 범위
//! 검사가 없으므로 이 사양은 32개 등록 entry의 원본 의미만 표현하며,
//! 다른 byte 값의 안전성이나 실제 도달을 주장하지 않는다.
//! sub00/01은 먼저 `advance_bytes`만큼 stream을 옮긴 뒤, 옮겨진
//! record의 `condition_offset` byte가 0이면 yield하고 0이 아니면
//! `pointer_offset`의 pointer로 다시 분기한다.

pub const QUEST_SEQ_OP28_HANDLER_RUNTIME: u32 = 0x8002_30F4;
pub const QUEST_SEQ_OP28_DISPATCH_RUNTIME: u32 = 0x8001_B0A8;
pub const QUEST_SEQ_OP28_DISPATCH_TABLE_RUNTIME: u32 = 0x800B_1A74;
pub const QUEST_SEQ_OP28_SUBOPCODE_COUNT: usize = 0x20;
pub const QUEST_SEQ_OP28_FALLBACK_HANDLER_RUNTIME: u32 = 0x8001_C3C8;

pub const QUEST_SEQ_OP28_DISPATCH_HANDLERS: [u32; QUEST_SEQ_OP28_SUBOPCODE_COUNT] = [
    0x8001_C3DC,
    0x8001_C448,
    0x8001_C4E4,
    0x8001_C518,
    0x8001_C57C,
    0x8001_C3C8,
    0x8001_C674,
    0x8001_C3C8,
    0x8001_C3C8,
    0x8001_C3C8,
    0x8001_C3C8,
    0x8001_C5E0,
    0x8001_C628,
    0x8001_C3C8,
    0x8001_C3C8,
    0x8001_C3C8,
    0x8001_8740,
    0x8001_C3C8,
    0x8001_88F0,
    0x8001_C3C8,
    0x8001_C3C8,
    0x8001_C3C8,
    0x8001_C3C8,
    0x8001_C3C8,
    0x8001_C688,
    0x8001_C774,
    0x8001_C8D0,
    0x8001_C3C8,
    0x8001_C3C8,
    0x8001_C3C8,
    0x8001_C3C8,
    0x8001_C3C8,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqOp28Flow {
    FixedAdvance {
        bytes: i16,
    },
    AdvanceOrBranch {
        advance_bytes: i16,
        condition_offset: i16,
        pointer_offset: i16,
    },
}

pub const QUEST_SEQ_OP28_FLOWS: [QuestSeqOp28Flow; QUEST_SEQ_OP28_SUBOPCODE_COUNT] = [
    QuestSeqOp28Flow::AdvanceOrBranch {
        advance_bytes: 8,
        condition_offset: 3,
        pointer_offset: 4,
    },
    QuestSeqOp28Flow::AdvanceOrBranch {
        advance_bytes: 12,
        condition_offset: 3,
        pointer_offset: 8,
    },
    fixed(4),
    fixed(8),
    fixed(8),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(8),
    fixed(8),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(16),
    fixed(4),
    fixed(16),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
    fixed(4),
];

const fn fixed(bytes: i16) -> QuestSeqOp28Flow {
    QuestSeqOp28Flow::FixedAdvance { bytes }
}

pub fn quest_seq_op28_handler(subopcode: u8) -> Option<u32> {
    QUEST_SEQ_OP28_DISPATCH_HANDLERS
        .get(usize::from(subopcode))
        .copied()
}

pub fn quest_seq_op28_flow(subopcode: u8) -> Option<QuestSeqOp28Flow> {
    QUEST_SEQ_OP28_FLOWS.get(usize::from(subopcode)).copied()
}

pub fn quest_seq_op28_unique_handler_count() -> usize {
    let mut handlers = QUEST_SEQ_OP28_DISPATCH_HANDLERS;
    handlers.sort_unstable();
    1 + handlers
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .count()
}

pub fn quest_seq_op28_fallback_entry_count() -> usize {
    QUEST_SEQ_OP28_DISPATCH_HANDLERS
        .iter()
        .filter(|&&handler| handler == QUEST_SEQ_OP28_FALLBACK_HANDLER_RUNTIME)
        .count()
}

#[cfg(test)]
#[path = "quest_seq_op28_tests.rs"]
mod tests;
