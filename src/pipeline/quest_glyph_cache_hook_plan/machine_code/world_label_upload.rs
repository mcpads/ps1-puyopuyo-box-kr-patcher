//! 월드 지명의 즉시 그리기는 공용 page 전체 대신 해당 문자열의 cell만 올린다.
//! 부분 전송은 resident state의 상위 bit로 구별한다. object marker는 원래 page를
//! 유지하고, 뒤따르는 공용 UI 요청은 기존 item page 교체 경로에서 전체를 보충한다.

use super::super::super::quest_glyph_cache_plan::QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX;
use super::super::QuestGlyphCacheHookPlanError;
use super::{patch_branch, push_address};
use crate::disasm::quest_ui_literals::{
    QUEST_WORLD_MAP_LABEL_TABLE_END_RUNTIME, QUEST_WORLD_MAP_LABEL_TABLE_RUNTIME,
    QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME,
};
use crate::font::QUEST_CODE_TABLE_RUNTIME;
use crate::isa::{InstructionKind as K, Reg};

// 72-byte 공용 frame의 마지막 word. decoder와 GPU helper를 넘어 보존한다.
const PARTIAL: i16 = 68;
const NOP: K = K::Sll {
    rd: Reg::ZERO,
    rt: Reg::ZERO,
    sa: 0,
};

#[cfg(test)]
#[path = "world_label_upload_tests.rs"]
mod tests;

pub(super) fn append_selection(k: &mut Vec<K>) -> Result<(), QuestGlyphCacheHookPlanError> {
    k.push(K::Sw {
        rt: Reg::ZERO,
        base: Reg::SP,
        offset: PARTIAL,
    });
    k.extend([
        K::Lw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 60,
        },
        NOP,
    ]);
    push_address(
        k,
        Reg::T1,
        QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME + 8,
    );
    let wrong_caller = k.len();
    k.extend([
        K::Bne {
            rs: Reg::T0,
            rt: Reg::T1,
            offset: 0,
        },
        NOP,
        K::Lw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 20,
        },
        NOP,
    ]);
    push_address(k, Reg::T1, QUEST_WORLD_MAP_LABEL_TABLE_RUNTIME);
    k.extend([
        K::Subu {
            rd: Reg::T1,
            rs: Reg::T0,
            rt: Reg::T1,
        },
        K::Sltiu {
            rt: Reg::T1,
            rs: Reg::T1,
            imm: (QUEST_WORLD_MAP_LABEL_TABLE_END_RUNTIME - QUEST_WORLD_MAP_LABEL_TABLE_RUNTIME)
                as i16,
        },
    ]);
    let table = k.len();
    k.extend([
        K::Bne {
            rs: Reg::T1,
            rt: Reg::ZERO,
            offset: 0,
        },
        NOP,
    ]);
    // 북쪽 강은 원본 consumer가 공용 table 밖 고정 literal을 선택한다.
    push_address(k, Reg::T1, 0x800B_E23C);
    let other_pointer = k.len();
    k.extend([
        K::Bne {
            rs: Reg::T0,
            rt: Reg::T1,
            offset: 0,
        },
        NOP,
    ]);
    let aligned = k.len();
    k.push(K::Andi {
        rt: Reg::T1,
        rs: Reg::T0,
        imm: 1,
    });
    let unaligned = k.len();
    k.extend([
        K::Bne {
            rs: Reg::T1,
            rt: Reg::ZERO,
            offset: 0,
        },
        NOP,
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: 1,
        },
        K::Sw {
            rt: Reg::T1,
            base: Reg::SP,
            offset: PARTIAL,
        },
    ]);
    let end = k.len();
    patch_branch(k, table, aligned)?;
    for b in [wrong_caller, other_pointer, unaligned] {
        patch_branch(k, b, end)?;
    }
    Ok(())
}

/// 미사용 cell을 건너뛸 branch를 반환한다. catalog 소비 뒤 S5는 공용 말미에서 진행한다.
pub(super) fn append_filter(k: &mut Vec<K>) -> Result<usize, QuestGlyphCacheHookPlanError> {
    k.extend([
        K::Lw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: PARTIAL,
        },
        NOP,
    ]);
    let full = k.len();
    k.extend([
        K::Beq {
            rs: Reg::T0,
            rt: Reg::ZERO,
            offset: 0,
        },
        NOP,
        K::Sll {
            rd: Reg::T0,
            rt: Reg::S5,
            sa: 1,
        },
    ]);
    push_address(
        k,
        Reg::T1,
        QUEST_CODE_TABLE_RUNTIME + (QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX * 2) as u32,
    );
    k.extend([
        K::Addu {
            rd: Reg::T0,
            rs: Reg::T1,
            rt: Reg::T0,
        },
        K::Lhu {
            rt: Reg::T3,
            base: Reg::T0,
            offset: 0,
        },
        K::Lw {
            rt: Reg::T1,
            base: Reg::SP,
            offset: 20,
        },
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::ZERO,
            imm: 16,
        },
    ]);
    let scan = k.len();
    k.extend([
        K::Lhu {
            rt: Reg::T4,
            base: Reg::T1,
            offset: 0,
        },
        NOP,
    ]);
    let nul = k.len();
    k.extend([
        K::Beq {
            rs: Reg::T4,
            rt: Reg::ZERO,
            offset: 0,
        },
        NOP,
    ]);
    let matched = k.len();
    k.extend([
        K::Beq {
            rs: Reg::T4,
            rt: Reg::T3,
            offset: 0,
        },
        NOP,
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::T2,
            imm: -1,
        },
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::T1,
            imm: 2,
        },
    ]);
    let more = k.len();
    k.extend([
        K::Bne {
            rs: Reg::T2,
            rt: Reg::ZERO,
            offset: 0,
        },
        NOP,
    ]);
    // 원본 고정 slot writer의 NUL/길이 검산이 선행한다. 유한 탐색 밖으로 읽지 않는다.
    let skip = k.len();
    k.extend([
        K::Beq {
            rs: Reg::ZERO,
            rt: Reg::ZERO,
            offset: 0,
        },
        NOP,
    ]);
    let decode = k.len();
    patch_branch(k, full, decode)?;
    patch_branch(k, matched, decode)?;
    patch_branch(k, nul, skip)?;
    patch_branch(k, more, scan)?;
    Ok(skip)
}

pub(super) fn append_partial_commit(k: &mut Vec<K>) {
    k.extend([
        K::Lw {
            rt: Reg::T3,
            base: Reg::SP,
            offset: PARTIAL,
        },
        NOP,
        K::Beq {
            rs: Reg::T3,
            rt: Reg::ZERO,
            offset: 2,
        },
        NOP,
        K::Ori {
            rt: Reg::S1,
            rs: Reg::S1,
            imm: 0x80,
        },
    ]);
}
