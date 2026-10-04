//! 도구 사용·포기 문장의 이름 draw 뒤에 받침에 맞는 목적격 조사 한 글자를 붙인다.
//!
//! 메뉴·획득 이름은 호출 return으로 제외한다. 기존 whole-string active scope
//! 안에서 그리며 원래 반환값과 frame을 보존한다. NUL 탐색은 64칸으로 제한한다.
use super::{patch_branch, push_address, QuestGlyphCacheHookPlanError};
use crate::disasm::QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME;
use crate::isa::{InstructionKind as K, Reg};

pub(super) fn append_item_object_particle(
    kinds: &mut Vec<K>,
    codes: &[(u16, u16)],
) -> Result<(), QuestGlyphCacheHookPlanError> {
    if codes.is_empty() {
        return Ok(());
    }
    let nop = K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    };
    kinds.extend([
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -24,
        },
        K::Sw {
            rt: Reg::V0,
            base: Reg::SP,
            offset: 16,
        },
        K::Lw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 84,
        },
    ]);
    push_address(kinds, Reg::T1, 0x8005_3390);
    let used = kinds.len();
    kinds.extend([
        K::Beq {
            rs: Reg::T0,
            rt: Reg::T1,
            offset: 0,
        },
        nop,
    ]);
    push_address(kinds, Reg::T1, 0x8006_18FC);
    let excluded = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T0,
            rt: Reg::T1,
            offset: 0,
        },
        nop,
    ]);
    let selected = kinds.len();
    patch_branch(kinds, used, selected)?;
    kinds.extend([
        K::Lw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 44,
        },
        K::Addiu {
            rt: Reg::T3,
            rs: Reg::ZERO,
            imm: 64,
        },
        K::Addu {
            rd: Reg::T2,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
    ]);
    let scan = kinds.len();
    kinds.extend([
        K::Lhu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 2,
        },
    ]);
    let terminated = kinds.len();
    kinds.extend([
        K::Beq {
            rs: Reg::T1,
            rt: Reg::ZERO,
            offset: 0,
        },
        nop,
        K::Addu {
            rd: Reg::T2,
            rs: Reg::T1,
            rt: Reg::ZERO,
        },
        K::Addiu {
            rt: Reg::T3,
            rs: Reg::T3,
            imm: -1,
        },
        K::Bne {
            rs: Reg::T3,
            rt: Reg::ZERO,
            offset: 0,
        },
        nop,
    ]);
    let repeat = kinds.len() - 2;
    patch_branch(kinds, repeat, scan)?;
    let unterminated = kinds.len();
    kinds.extend([
        K::Beq {
            rs: Reg::ZERO,
            rt: Reg::ZERO,
            offset: 0,
        },
        nop,
    ]);
    let classify = kinds.len();
    patch_branch(kinds, terminated, classify)?;
    let mut selected = Vec::new();
    for &(tail, particle) in codes {
        kinds.push(K::Ori {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: tail,
        });
        selected.push(kinds.len());
        kinds.extend([
            K::Beq {
                rs: Reg::T2,
                rt: Reg::T1,
                offset: 0,
            },
            K::Ori {
                rt: Reg::T4,
                rs: Reg::ZERO,
                imm: particle,
            },
        ]);
    }
    let unknown = kinds.len();
    kinds.extend([
        K::Beq {
            rs: Reg::ZERO,
            rt: Reg::ZERO,
            offset: 0,
        },
        nop,
    ]);
    let draw = kinds.len();
    // word store가 조사 u16과 NUL을 동시에 만든다. 0..15는 callee argument 영역이다.
    kinds.extend([
        K::Sw {
            rt: Reg::T4,
            base: Reg::SP,
            offset: 20,
        },
        K::Addu {
            rd: Reg::A0,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        K::Addiu {
            rt: Reg::A1,
            rs: Reg::SP,
            imm: 20,
        },
        K::Jal {
            target: QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME,
        },
        nop,
    ]);
    let done = kinds.len();
    kinds.extend([
        K::Lw {
            rt: Reg::V0,
            base: Reg::SP,
            offset: 16,
        },
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: 24,
        },
    ]);
    for branch in selected {
        patch_branch(kinds, branch, draw)?;
    }
    for branch in [excluded, unterminated, unknown] {
        patch_branch(kinds, branch, done)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "item_object_particle_tests.rs"]
mod tests;

/// 버리기 확인의 마지막 이름 glyph만 두 글자 NUL 문자열로 출력한다.
/// 원본 loop의 S0(다음 byte offset) >= S1(byte 길이)일 때만 들어간다.
pub(super) fn append_discard_object_particle(
    kinds: &mut Vec<K>,
    codes: &[(u16, u16)],
) -> Result<(), QuestGlyphCacheHookPlanError> {
    if codes.is_empty() {
        return Ok(());
    }
    let nop = K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    };
    kinds.push(K::Lw {
        rt: Reg::T0,
        base: Reg::SP,
        offset: 60,
    });
    push_address(kinds, Reg::T1, 0x8003_7C60);
    let excluded = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T0,
            rt: Reg::T1,
            offset: 0,
        },
        nop,
        K::Lw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 52,
        },
        K::Lw {
            rt: Reg::T1,
            base: Reg::SP,
            offset: 48,
        },
        nop,
        K::Sltu {
            rd: Reg::T0,
            rs: Reg::T0,
            rt: Reg::T1,
        },
    ]);
    let unfinished = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T0,
            rt: Reg::ZERO,
            offset: 0,
        },
        nop,
        K::Lw {
            rt: Reg::T2,
            base: Reg::SP,
            offset: 20,
        },
        nop,
    ]);
    let mut selected = Vec::new();
    for &(tail, particle) in codes {
        kinds.push(K::Ori {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: tail,
        });
        selected.push(kinds.len());
        kinds.extend([
            K::Beq {
                rs: Reg::T2,
                rt: Reg::T1,
                offset: 0,
            },
            K::Ori {
                rt: Reg::T4,
                rs: Reg::ZERO,
                imm: particle,
            },
        ]);
    }
    let unknown = kinds.len();
    kinds.extend([
        K::Beq {
            rs: Reg::ZERO,
            rt: Reg::ZERO,
            offset: 0,
        },
        nop,
    ]);
    let draw = kinds.len();
    kinds.extend([
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -24,
        },
        K::Sh {
            rt: Reg::T2,
            base: Reg::SP,
            offset: 16,
        },
        K::Sh {
            rt: Reg::T4,
            base: Reg::SP,
            offset: 18,
        },
        K::Sh {
            rt: Reg::ZERO,
            base: Reg::SP,
            offset: 20,
        },
        K::Lw {
            rt: Reg::A0,
            base: Reg::SP,
            offset: 80,
        },
        K::Addiu {
            rt: Reg::A1,
            rs: Reg::SP,
            imm: 16,
        },
    ]);
    use crate::pipeline::quest_item_glyph_cache_dispatch::{
        push_restore_frame, QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME,
    };
    push_address(kinds, Reg::T0, QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME);
    kinds.extend([
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: 1,
        },
        K::Sb {
            rt: Reg::T1,
            base: Reg::T0,
            offset: 0,
        },
        K::Jal {
            target: QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME,
        },
        nop,
    ]);
    push_address(kinds, Reg::T0, QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME);
    kinds.extend([
        K::Sb {
            rt: Reg::ZERO,
            base: Reg::T0,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: 24,
        },
    ]);
    push_restore_frame(kinds);
    kinds.extend([K::Jr { rs: Reg::RA }, nop]);
    let fallback = kinds.len();
    for branch in selected {
        patch_branch(kinds, branch, draw)?;
    }
    for branch in [excluded, unfinished, unknown] {
        patch_branch(kinds, branch, fallback)?;
    }
    Ok(())
}
