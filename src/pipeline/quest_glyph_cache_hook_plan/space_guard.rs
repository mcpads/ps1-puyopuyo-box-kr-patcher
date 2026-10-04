//! 코드 8140에 배정한 한글이 원본 공백 fast path로 사라지지 않게 한다.
use super::{error, QuestGlyphCacheHookPlanError, QUEST_GLYPH_CACHE_RENDERER_PAGE_RUNTIME};
use crate::coords::ExeMap;
use crate::isa::{verify_typed_source, InstructionKind as K, IsaProfile, Reg, VerifiedMachineCode};

pub(super) const SITE_RUNTIME: u32 = 0x8002_BA8C;
const SOURCE_ID: &str = "pipeline::quest_glyph_cache_hook_plan::space_guard";

fn original() -> [K; 2] {
    [
        K::Lhu {
            rt: Reg::V1,
            base: Reg::V0,
            offset: -0x1f7c,
        },
        K::Andi {
            rt: Reg::V0,
            rs: Reg::S1,
            imm: 0xffff,
        },
    ]
}

pub(super) fn assemble(
    map: &ExeMap,
    text: &[u8],
    entry: u32,
) -> Result<VerifiedMachineCode, QuestGlyphCacheHookPlanError> {
    let expected = verify_typed_source(IsaProfile::DECLARED, SITE_RUNTIME, SOURCE_ID, &original())?;
    let start = map
        .text_offset(SITE_RUNTIME)
        .ok_or_else(|| error("QUEST 공백 분기 좌표 없음"))?;
    if text.get(start..start + expected.bytes().len()) != Some(expected.bytes()) {
        return Err(error("QUEST 공백 분기 원본 명령 불일치"));
    }
    Ok(verify_typed_source(
        IsaProfile::DECLARED,
        SITE_RUNTIME,
        SOURCE_ID,
        &[
            K::J { target: entry },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ],
    )?)
}

pub(super) fn append(
    kinds: &mut Vec<K>,
    space_slots: &[Option<u16>],
) -> Result<usize, QuestGlyphCacheHookPlanError> {
    let entry = kinds.len();
    kinds.extend(original());
    super::machine_code::push_address(kinds, Reg::T0, QUEST_GLYPH_CACHE_RENDERER_PAGE_RUNTIME);
    kinds.extend([
        K::Lbu {
            rt: Reg::T0,
            base: Reg::T0,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    kinds.push(K::Andi {
        rt: Reg::T0,
        rs: Reg::T0,
        imm: 0x7f,
    });
    let mut remapped_pages = Vec::new();
    for (page, slot) in space_slots.iter().enumerate() {
        // 공백을 원본 292번 slot에 남긴 page는 빈 cell 업로드를 보장하지 않는다.
        // 그 page의 8140은 원래 fast path를 유지해야 이전 page의 잔상이 안 나온다.
        if *slot == Some(292) {
            continue;
        }
        kinds.push(K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: (page + 1) as i16,
        });
        remapped_pages.push(kinds.len());
        kinds.extend([
            K::Beq {
                rs: Reg::T0,
                rt: Reg::T1,
                offset: 0,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ]);
    }
    kinds.extend([
        K::J {
            target: SITE_RUNTIME + 8,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let remapped = kinds.len();
    kinds.extend([
        // 재배정된 page의 공백 폭과 blank cell은 page별 공급이 소유한다.
        K::Ori {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 0xffff,
        },
    ]);
    kinds.extend([
        K::J {
            target: SITE_RUNTIME + 8,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    for branch in remapped_pages {
        super::machine_code::patch_branch(kinds, branch, remapped)?;
    }
    Ok(entry)
}

#[cfg(test)]
#[path = "space_guard_tests.rs"]
mod tests;
