//! 공용 glyph page 요청 frame과 page ID 정규화 진입 조립.
//!
//! OP21 inline/relocated record가 공유하는 frame ABI를 만들고 유효 page만
//! bank 선택 단계로 보낸다. 원본 page 복귀 target은 뒤 단계가 확정한 뒤
//! 하나의 외부 fixup으로 연결한다.

use crate::isa::{InstructionKind as K, Reg};
use crate::script::TOWN00_SEQ_RUNTIME_BASE;

use super::super::{
    QuestGlyphCacheHookPlanError, QUEST_GLYPH_CACHE_ASSIGNMENT_MODE_STACK_OFFSET,
    QUEST_GLYPH_CACHE_FRAME_BYTES,
};
use super::{patch_branch, push_address};

#[derive(Debug, Clone, Copy)]
pub(super) struct PageRequestFixups {
    route_inline_branch: usize,
    route_ready: usize,
    page_valid_branch: usize,
    page_valid: usize,
    original_page_branch: usize,
}

impl PageRequestFixups {
    pub(super) fn entry(self) -> usize {
        self.page_valid
    }
}

pub(super) fn append_page_request_entry(
    kinds: &mut Vec<K>,
    page_limit_exclusive: i16,
) -> PageRequestFixups {
    kinds.extend([
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -QUEST_GLYPH_CACHE_FRAME_BYTES,
        },
        K::Sw {
            rt: Reg::RA,
            base: Reg::SP,
            offset: 60,
        },
        K::Sw {
            rt: Reg::A0,
            base: Reg::SP,
            offset: 56,
        },
        K::Sw {
            rt: Reg::S0,
            base: Reg::SP,
            offset: 52,
        },
        K::Sw {
            rt: Reg::S1,
            base: Reg::SP,
            offset: 48,
        },
        K::Sw {
            rt: Reg::S2,
            base: Reg::SP,
            offset: 44,
        },
        K::Sw {
            rt: Reg::S3,
            base: Reg::SP,
            offset: 40,
        },
        K::Sw {
            rt: Reg::S4,
            base: Reg::SP,
            offset: 36,
        },
        K::Sw {
            rt: Reg::S5,
            base: Reg::SP,
            offset: 32,
        },
        K::Sw {
            rt: Reg::S6,
            base: Reg::SP,
            offset: 28,
        },
        K::Sw {
            rt: Reg::S7,
            base: Reg::SP,
            offset: 24,
        },
        K::Sw {
            rt: Reg::A1,
            base: Reg::SP,
            offset: 20,
        },
        K::Sw {
            rt: Reg::A2,
            base: Reg::SP,
            offset: 16,
        },
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::SP,
            offset: 12,
        },
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::SP,
            offset: 8,
        },
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::SP,
            offset: QUEST_GLYPH_CACHE_ASSIGNMENT_MODE_STACK_OFFSET,
        },
        K::Lw {
            rt: Reg::S0,
            base: Reg::A0,
            offset: 0x1C,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lbu {
            rt: Reg::S1,
            base: Reg::S0,
            offset: 1,
        },
        K::Lhu {
            rt: Reg::T0,
            base: Reg::S0,
            offset: 2,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Beq {
            rs: Reg::T0,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::T0,
            rt: Reg::T0,
            sa: 2,
        },
    ]);
    let route_inline_branch = kinds.len() - 2;
    push_address(kinds, Reg::T1, TOWN00_SEQ_RUNTIME_BASE);
    kinds.extend([
        K::Addu {
            rd: Reg::S0,
            rs: Reg::T1,
            rt: Reg::T0,
        },
        K::Addiu {
            rt: Reg::S0,
            rs: Reg::S0,
            imm: -4,
        },
    ]);
    let route_ready = kinds.len();
    kinds.extend([
        K::Sltiu {
            rt: Reg::T0,
            rs: Reg::S1,
            imm: page_limit_exclusive,
        },
        K::Bne {
            rs: Reg::T0,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Addu {
            rd: Reg::S1,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
    ]);
    let page_valid_branch = route_ready + 1;
    let page_valid = kinds.len();
    kinds.extend([
        K::Beq {
            rs: Reg::S1,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let original_page_branch = page_valid;

    PageRequestFixups {
        route_inline_branch,
        route_ready,
        page_valid_branch,
        page_valid,
        original_page_branch,
    }
}

pub(super) fn patch_page_request_routes(
    kinds: &mut [K],
    fixups: PageRequestFixups,
    return_with_route_marker: usize,
) -> Result<(), QuestGlyphCacheHookPlanError> {
    for (source, target) in [
        (fixups.route_inline_branch, fixups.route_ready),
        (fixups.page_valid_branch, fixups.page_valid),
        (fixups.original_page_branch, return_with_route_marker),
    ] {
        patch_branch(kinds, source, target)?;
    }
    Ok(())
}
