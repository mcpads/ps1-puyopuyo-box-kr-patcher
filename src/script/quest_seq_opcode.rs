//! `QUEST.EXE`가 사용하는 SEQ opcode dispatch 표.
//!
//! 공용 base 표가 `0x00..0x7F`를 소유하고, QUEST의 두 등록 표가
//! `0x80..0xDF`, `0xE0..0xFF`를 덮어쓴다. 여러 opcode가 공용 fallback
//! handler를 가리키지만, 이것만으로 opcode의 실행 도달성이나 record
//! 폭을 판정하지 않는다.

pub const QUEST_SEQ_BASE_OPCODE_COUNT: usize = 0x80;
pub const QUEST_SEQ_BASE_FALLBACK_HANDLER_RUNTIME: u32 = 0x8002_263C;
pub const QUEST_SEQ_OPCODE_BASE: u8 = 0x80;
pub const QUEST_SEQ_OPCODE_COUNT: usize = 0x80;
pub const QUEST_SEQ_MAIN_OPCODE_COUNT: usize = 0x60;
pub const QUEST_SEQ_EXTENDED_OPCODE_BASE: u8 = 0xE0;
pub const QUEST_SEQ_EXTENDED_OPCODE_COUNT: usize = 0x20;
pub const QUEST_SEQ_MAIN_FALLBACK_HANDLER_RUNTIME: u32 = 0x8007_80DC;
pub const QUEST_SEQ_EXTENDED_FALLBACK_HANDLER_RUNTIME: u32 = 0x8007_00A0;

/// opcode `0x00..0x7F` 순서의 공용 base handler runtime 주소.
pub const QUEST_SEQ_BASE_DISPATCH_HANDLERS: [u32; QUEST_SEQ_BASE_OPCODE_COUNT] = [
    0x8002_2650,
    0x8002_268C,
    0x8002_26A4,
    0x8002_2700,
    0x8002_2714,
    0x8002_2748,
    0x8002_2770,
    0x8002_27D0,
    0x8001_FFF0,
    0x8002_28BC,
    0x8002_28EC,
    0x8002_0188,
    0x8002_0278,
    0x8002_2948,
    0x8002_297C,
    0x8002_0430,
    0x8002_0730,
    0x8002_0A48,
    0x8002_0D40,
    0x8002_0F44,
    0x8002_1160,
    0x8002_137C,
    0x8002_1670,
    0x8002_1964,
    0x8002_1C40,
    0x8002_29E0,
    0x8002_29F4,
    0x8002_2B30,
    0x8002_2C7C,
    0x8002_2CD4,
    0x8002_2CF8,
    0x8002_2D3C,
    0x8002_2DB8,
    0x8002_2F0C,
    0x8002_2F24,
    0x8002_2F44,
    0x8002_2F78,
    0x8002_2FA0,
    0x8002_2FE4,
    0x8002_3038,
    0x8002_30F4,
    0x8002_3114,
    0x8002_31A0,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_31F4,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_32DC,
    0x8002_1F70,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
    0x8002_263C,
];

/// opcode `0x80..0xFF` 순서의 원본 handler runtime 주소.
pub const QUEST_SEQ_DISPATCH_HANDLERS: [u32; QUEST_SEQ_OPCODE_COUNT] = [
    0x8007_80E4,
    0x8007_81A8,
    0x8007_2844,
    0x8007_33A4,
    0x8007_81D8,
    0x8007_8280,
    0x8007_8360,
    0x8007_35D0,
    0x8007_83C0,
    0x8007_3960,
    0x8007_3A9C,
    0x8007_3DE8,
    0x8007_8448,
    0x8007_84BC,
    0x8007_41C8,
    0x8007_8540,
    0x8007_81F8,
    0x8007_85C4,
    0x8007_8614,
    0x8007_8650,
    0x8007_8744,
    0x8007_87CC,
    0x8007_87F4,
    0x8007_42FC,
    0x8007_445C,
    0x8007_45B0,
    0x8007_8838,
    0x8007_4700,
    0x8007_4858,
    0x8007_88AC,
    0x8007_891C,
    0x8007_89D0,
    0x8007_49A8,
    0x8007_8A40,
    0x8007_8AA0,
    0x8007_8B28,
    0x8007_8B70,
    0x8007_4AD0,
    0x8007_8C64,
    0x8007_8CA4,
    0x8007_8CDC,
    0x8007_8D0C,
    0x8007_4CEC,
    0x8007_8D60,
    0x8007_50C0,
    0x8007_8DC8,
    0x8007_55D0,
    0x8007_8E14,
    0x8007_8F30,
    0x8007_5A4C,
    0x8007_5C24,
    0x8007_8F5C,
    0x8007_903C,
    0x8007_90F0,
    0x8007_91D8,
    0x8007_5D2C,
    0x8007_9284,
    0x8007_92B8,
    0x8007_5EE8,
    0x8007_92FC,
    0x8007_93D0,
    0x8007_9454,
    0x8007_9498,
    0x8007_62CC,
    0x8007_66B8,
    0x8007_9658,
    0x8007_96AC,
    0x8007_67DC,
    0x8007_69E4,
    0x8007_6AC0,
    0x8007_9884,
    0x8007_98CC,
    0x8007_6C28,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8007_80DC,
    0x8006_FDB8,
    0x8006_F620,
    0x8006_FED0,
    0x8006_FF34,
    0x8007_0054,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
    0x8007_00A0,
];

pub fn quest_seq_opcode_handler(opcode: u8) -> Option<u32> {
    if opcode < QUEST_SEQ_OPCODE_BASE {
        return QUEST_SEQ_BASE_DISPATCH_HANDLERS
            .get(usize::from(opcode))
            .copied();
    }
    QUEST_SEQ_DISPATCH_HANDLERS
        .get(usize::from(opcode - QUEST_SEQ_OPCODE_BASE))
        .copied()
}

pub fn quest_seq_base_dispatch_unique_handlers() -> usize {
    QUEST_SEQ_BASE_DISPATCH_HANDLERS
        .iter()
        .enumerate()
        .filter(|(index, handler)| !QUEST_SEQ_BASE_DISPATCH_HANDLERS[..*index].contains(handler))
        .count()
}

pub fn quest_seq_base_dispatch_fallback_entries() -> usize {
    QUEST_SEQ_BASE_DISPATCH_HANDLERS
        .iter()
        .filter(|&&handler| handler == QUEST_SEQ_BASE_FALLBACK_HANDLER_RUNTIME)
        .count()
}

pub fn quest_seq_dispatch_unique_handlers() -> usize {
    QUEST_SEQ_DISPATCH_HANDLERS
        .iter()
        .enumerate()
        .filter(|(index, handler)| !QUEST_SEQ_DISPATCH_HANDLERS[..*index].contains(handler))
        .count()
}

pub fn quest_seq_dispatch_fallback_entries() -> usize {
    QUEST_SEQ_DISPATCH_HANDLERS
        .iter()
        .filter(|&&handler| {
            handler == QUEST_SEQ_MAIN_FALLBACK_HANDLER_RUNTIME
                || handler == QUEST_SEQ_EXTENDED_FALLBACK_HANDLER_RUNTIME
        })
        .count()
}

#[cfg(test)]
#[path = "quest_seq_opcode_tests.rs"]
mod tests;
