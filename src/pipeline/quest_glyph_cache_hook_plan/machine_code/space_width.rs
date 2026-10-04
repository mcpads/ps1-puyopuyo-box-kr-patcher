//! page와 slot이 일치하는 공백만 반각으로 표시한다.
use super::{patch_branch, QuestGlyphCacheHookPlanError};
use crate::isa::{InstructionKind as K, Reg};

pub(super) fn append_space_width(
    kinds: &mut Vec<K>,
    slots: &[Option<u16>],
) -> Result<(), QuestGlyphCacheHookPlanError> {
    if slots.is_empty() {
        return Ok(());
    }
    let nop = K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    };
    kinds.extend([
        K::Andi {
            rt: Reg::T2,
            rs: Reg::T0,
            imm: 0x7f,
        },
        K::Lhu {
            rt: Reg::T3,
            base: Reg::A2,
            offset: 0x66,
        },
    ]);
    let mut start = 0;
    let mut matches = Vec::new();
    while start < slots.len() {
        let mut end = start + 1;
        while end < slots.len() && slots[end] == slots[start] {
            end += 1;
        }
        if let Some(slot) = slots[start] {
            let skip;
            if end == start + 1 {
                kinds.push(K::Addiu {
                    rt: Reg::AT,
                    rs: Reg::ZERO,
                    imm: (start + 1) as i16,
                });
                skip = kinds.len();
                kinds.push(K::Bne {
                    rs: Reg::T2,
                    rt: Reg::AT,
                    offset: 0,
                });
            } else {
                kinds.extend([
                    K::Addiu {
                        rt: Reg::AT,
                        rs: Reg::T2,
                        imm: -((start + 1) as i16),
                    },
                    K::Sltiu {
                        rt: Reg::AT,
                        rs: Reg::AT,
                        imm: (end - start) as i16,
                    },
                ]);
                skip = kinds.len();
                kinds.push(K::Beq {
                    rs: Reg::AT,
                    rt: Reg::ZERO,
                    offset: 0,
                });
            }
            // page 분기의 delay slot에서 slot 상수를 준비한다.
            kinds.push(K::Addiu {
                rt: Reg::AT,
                rs: Reg::ZERO,
                imm: slot as i16,
            });
            matches.push(kinds.len());
            kinds.extend([
                K::Beq {
                    rs: Reg::T3,
                    rt: Reg::AT,
                    offset: 0,
                },
                nop,
            ]);
            let next = kinds.len();
            patch_branch(kinds, skip, next)?;
        }
        start = end;
    }
    let unmatched = kinds.len();
    kinds.extend([
        K::Beq {
            rs: Reg::ZERO,
            rt: Reg::ZERO,
            offset: 0,
        },
        nop,
    ]);
    let narrow = kinds.len();
    kinds.push(K::Sra {
        rd: Reg::T1,
        rt: Reg::T1,
        sa: 1,
    });
    let finish = kinds.len();
    patch_branch(kinds, unmatched, finish)?;
    for branch in matches {
        patch_branch(kinds, branch, narrow)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "space_width_tests.rs"]
mod tests;
