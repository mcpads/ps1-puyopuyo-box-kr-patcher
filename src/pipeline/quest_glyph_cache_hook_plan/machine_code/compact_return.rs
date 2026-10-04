//! compact 복귀는 text 활성화와 page 요청을 우회하고 SEQ cursor만 갱신한다.
use super::super::QuestGlyphCacheHookPlanError;
use super::{patch_branch, push_address};
use crate::isa::{InstructionKind as K, Reg};
use crate::pipeline::quest_compact_return::QUEST_COMPACT_RETURN_MARKER;
use crate::script::QUEST_SEQ_ACTIVE_RUNTIME_BASE;

pub(super) fn append_compact_return(
    kinds: &mut Vec<K>,
) -> Result<(), QuestGlyphCacheHookPlanError> {
    let nop = K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    };
    kinds.extend([
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
        nop,
        K::Lbu {
            rt: Reg::T0,
            base: Reg::V0,
            offset: 1,
        },
        nop,
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_COMPACT_RETURN_MARKER),
        },
    ]);
    let normal = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T0,
            rt: Reg::T1,
            offset: 0,
        },
        nop,
        K::Lhu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 2,
        },
        nop,
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ]);
    push_address(kinds, Reg::T0, QUEST_SEQ_ACTIVE_RUNTIME_BASE);
    kinds.extend([
        K::Addu {
            rd: Reg::V0,
            rs: Reg::T0,
            rt: Reg::V0,
        },
        K::Jr { rs: Reg::RA },
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ]);
    let next = kinds.len();
    patch_branch(kinds, normal, next)
}
#[cfg(test)]
#[path = "compact_return_tests.rs"]
mod tests;
