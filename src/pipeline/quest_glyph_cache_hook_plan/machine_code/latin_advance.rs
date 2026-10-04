//! 보통 표시 효과의 영문·반각 숫자 cursor 간격을 줄인다. object 모양은 보존한다.
use super::{patch_branch, QuestGlyphCacheHookPlanError};
use crate::isa::{InstructionKind as K, Reg};

pub(super) fn append_latin_advance(
    kinds: &mut Vec<K>,
    pages: &[Vec<(u16, i16)>],
) -> Result<(), QuestGlyphCacheHookPlanError> {
    if pages.iter().all(Vec::is_empty) {
        return Ok(());
    }
    let nop = K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    };
    kinds.extend([
        K::Lbu {
            rt: Reg::T2,
            base: Reg::V1,
            offset: 0x18,
        },
        nop,
    ]);
    let effect = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T2,
            rt: Reg::ZERO,
            offset: 0,
        },
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
    let mut done = vec![effect];
    for (page, slots) in pages
        .iter()
        .enumerate()
        .filter(|(_, slots)| !slots.is_empty())
    {
        kinds.push(K::Addiu {
            rt: Reg::AT,
            rs: Reg::ZERO,
            imm: (page + 1) as i16,
        });
        let other_page = kinds.len();
        kinds.extend([
            K::Bne {
                rs: Reg::T2,
                rt: Reg::AT,
                offset: 0,
            },
            nop,
        ]);
        let mut index = 0;
        while index < slots.len() {
            let (slot, delta) = slots[index];
            let mut end = index + 1;
            while end < slots.len()
                && slots[end].1 == delta
                && slots[end - 1].0.checked_add(1) == Some(slots[end].0)
            {
                end += 1;
            }
            // 같은 보정의 연속 slot만 묶는다. 사이의 다른 문자는 포함하지 않는다.
            let range = end - index;
            if range == 1 {
                kinds.push(K::Addiu {
                    rt: Reg::AT,
                    rs: Reg::ZERO,
                    imm: slot as i16,
                });
            } else {
                kinds.extend([
                    K::Addiu {
                        rt: Reg::AT,
                        rs: Reg::T3,
                        imm: -(slot as i16),
                    },
                    K::Sltiu {
                        rt: Reg::AT,
                        rs: Reg::AT,
                        imm: range as i16,
                    },
                ]);
            }
            let other_slot = kinds.len();
            kinds.extend([
                if range == 1 {
                    K::Bne {
                        rs: Reg::T3,
                        rt: Reg::AT,
                        offset: 0,
                    }
                } else {
                    K::Beq {
                        rs: Reg::AT,
                        rt: Reg::ZERO,
                        offset: 0,
                    }
                },
                nop,
                K::Lhu {
                    rt: Reg::AT,
                    base: Reg::V1,
                    offset: 0x0A,
                },
                nop,
                K::Addiu {
                    rt: Reg::AT,
                    rs: Reg::AT,
                    imm: delta,
                },
                K::Sh {
                    rt: Reg::AT,
                    base: Reg::V1,
                    offset: 0x0A,
                },
            ]);
            done.push(kinds.len());
            kinds.extend([
                K::Beq {
                    rs: Reg::ZERO,
                    rt: Reg::ZERO,
                    offset: 0,
                },
                nop,
            ]);
            let next = kinds.len();
            patch_branch(kinds, other_slot, next)?;
            index = end;
        }
        let next = kinds.len();
        patch_branch(kinds, other_page, next)?;
    }
    let finish = kinds.len();
    for branch in done {
        patch_branch(kinds, branch, finish)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "latin_advance_tests.rs"]
mod tests;
