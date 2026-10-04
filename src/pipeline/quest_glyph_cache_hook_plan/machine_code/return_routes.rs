//! 공용 loader의 성공 복귀·재시도와 원본 renderer 복귀 조립.
//!
//! page 공급 결과를 OP21 또는 ITEM_DAT 호출 ABI에 맞게 되돌리고, 공급할
//! bank가 없거나 decode가 실패하면 stream을 소비하지 않은 채 재시도한다.
//! item별 복귀 target 선택은 별도 item route stage가 제공한다.

use crate::disasm::{
    QUEST_FONT_HOOK_CONTINUATION_RUNTIME, QUEST_GLYPH_RENDERER_CONTINUATION_RUNTIME,
    QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET,
};
use crate::isa::{InstructionKind as K, Reg};

use super::super::{
    QuestGlyphCacheHookPlanError, QUEST_GLYPH_CACHE_FRAME_BYTES,
    QUEST_GLYPH_CACHE_ROUTE_PAGE_RUNTIME,
};
use super::{patch_branch, push_address};

#[derive(Debug, Clone, Copy)]
pub(super) struct LoaderReturnFixups {
    return_with_route_marker: usize,
    yield_without_consuming: usize,
    return_original_font_branch: usize,
    yield_original_font_branch: usize,
}

impl LoaderReturnFixups {
    pub(super) fn successful_return(self) -> usize {
        self.return_with_route_marker
    }

    pub(super) fn retry_return(self) -> usize {
        self.yield_without_consuming
    }
}

fn push_restore_frame_registers(kinds: &mut Vec<K>) {
    kinds.extend([
        K::Lw {
            rt: Reg::A0,
            base: Reg::SP,
            offset: 56,
        },
        K::Lw {
            rt: Reg::RA,
            base: Reg::SP,
            offset: 60,
        },
        K::Lw {
            rt: Reg::S1,
            base: Reg::SP,
            offset: 48,
        },
        K::Lw {
            rt: Reg::S2,
            base: Reg::SP,
            offset: 44,
        },
        K::Lw {
            rt: Reg::S3,
            base: Reg::SP,
            offset: 40,
        },
        K::Lw {
            rt: Reg::S4,
            base: Reg::SP,
            offset: 36,
        },
        K::Lw {
            rt: Reg::S5,
            base: Reg::SP,
            offset: 32,
        },
        K::Lw {
            rt: Reg::S6,
            base: Reg::SP,
            offset: 28,
        },
        K::Lw {
            rt: Reg::S7,
            base: Reg::SP,
            offset: 24,
        },
    ]);
}

pub(super) fn append_loader_return_routes(kinds: &mut Vec<K>) -> LoaderReturnFixups {
    let return_with_route_marker = kinds.len();
    push_address(kinds, Reg::T0, QUEST_GLYPH_CACHE_ROUTE_PAGE_RUNTIME);
    kinds.extend([
        K::Lw {
            rt: Reg::T1,
            base: Reg::SP,
            offset: 8,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Sb {
            rt: Reg::T1,
            base: Reg::T0,
            offset: 0,
        },
    ]);

    let return_original_tail = kinds.len();
    kinds.extend([
        K::Lw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 12,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
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
    ]);
    let return_original_font_branch = return_original_tail + 2;
    push_restore_frame_registers(kinds);
    kinds.extend([
        K::Addu {
            rd: Reg::V0,
            rs: Reg::S0,
            rt: Reg::ZERO,
        },
        K::Lw {
            rt: Reg::S0,
            base: Reg::SP,
            offset: 52,
        },
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: QUEST_GLYPH_CACHE_FRAME_BYTES,
        },
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 1,
        },
        K::J {
            target: QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    let yield_without_consuming = kinds.len();
    kinds.extend([
        K::Lw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 12,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
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
    ]);
    let yield_original_font_branch = yield_without_consuming + 2;
    push_restore_frame_registers(kinds);
    kinds.extend([
        K::Lw {
            rt: Reg::S0,
            base: Reg::SP,
            offset: 52,
        },
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: QUEST_GLYPH_CACHE_FRAME_BYTES,
        },
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 1,
        },
        K::Sw {
            rt: Reg::V0,
            base: Reg::GP,
            offset: QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET,
        },
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    LoaderReturnFixups {
        return_with_route_marker,
        yield_without_consuming,
        return_original_font_branch,
        yield_original_font_branch,
    }
}

pub(super) fn append_original_font_guard_return(kinds: &mut Vec<K>) -> usize {
    let original_font_guard_return = kinds.len();
    kinds.extend([
        K::Lw {
            rt: Reg::A0,
            base: Reg::SP,
            offset: 56,
        },
        K::Lw {
            rt: Reg::A1,
            base: Reg::SP,
            offset: 20,
        },
        K::Lw {
            rt: Reg::A2,
            base: Reg::SP,
            offset: 16,
        },
        K::Lw {
            rt: Reg::RA,
            base: Reg::SP,
            offset: 60,
        },
        K::Lw {
            rt: Reg::S0,
            base: Reg::SP,
            offset: 52,
        },
        K::Lw {
            rt: Reg::S1,
            base: Reg::SP,
            offset: 48,
        },
        K::Lw {
            rt: Reg::S2,
            base: Reg::SP,
            offset: 44,
        },
        K::Lw {
            rt: Reg::S3,
            base: Reg::SP,
            offset: 40,
        },
        K::Lw {
            rt: Reg::S4,
            base: Reg::SP,
            offset: 36,
        },
        K::Lw {
            rt: Reg::S5,
            base: Reg::SP,
            offset: 32,
        },
        K::Lw {
            rt: Reg::S6,
            base: Reg::SP,
            offset: 28,
        },
        K::Lw {
            rt: Reg::S7,
            base: Reg::SP,
            offset: 24,
        },
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: QUEST_GLYPH_CACHE_FRAME_BYTES,
        },
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -32,
        },
        K::J {
            target: QUEST_GLYPH_RENDERER_CONTINUATION_RUNTIME,
        },
        K::Sw {
            rt: Reg::S0,
            base: Reg::SP,
            offset: 16,
        },
    ]);

    original_font_guard_return
}

pub(super) fn patch_loader_return_routes(
    kinds: &mut [K],
    fixups: LoaderReturnFixups,
    successful_non_op21_return: usize,
    failed_non_op21_return: usize,
) -> Result<(), QuestGlyphCacheHookPlanError> {
    patch_branch(
        kinds,
        fixups.return_original_font_branch,
        successful_non_op21_return,
    )?;
    patch_branch(
        kinds,
        fixups.yield_original_font_branch,
        failed_non_op21_return,
    )?;
    Ok(())
}
