//! SPRT가 앞선 한글 FT4의 GPU texture page를 이어받지 않도록 한다.
//!
//! SPRT의 UV·크기를 유지하면서 DR_TPAGE를 먼저 실행하는 OT 연결로 보낸다.

use crate::coords::ExeMap;
use crate::isa::{verify_typed_source, InstructionKind as K, IsaProfile, Reg, VerifiedMachineCode};

use super::{error, QuestGlyphCacheHookPlanError};

pub(super) const SITE_RUNTIME: u32 = 0x8002_B708;
const SPRITE_LINK_RUNTIME: u32 = 0x8002_B8CC;
const SOURCE_ID: &str = "pipeline::quest_glyph_cache_hook_plan::explicit_texture_page";

pub(super) fn assemble(
    map: &ExeMap,
    text: &[u8],
    entry_runtime: u32,
) -> Result<VerifiedMachineCode, QuestGlyphCacheHookPlanError> {
    let delay = K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    };
    let original = verify_typed_source(
        IsaProfile::DECLARED,
        SITE_RUNTIME,
        SOURCE_ID,
        &[
            K::J {
                target: SPRITE_LINK_RUNTIME,
            },
            delay,
        ],
    )?;
    let start = map
        .text_offset(SITE_RUNTIME)
        .ok_or_else(|| error("QUEST font 분기 좌표가 없음"))?;
    if text.get(start..start + original.bytes().len()) != Some(original.bytes()) {
        return Err(error("QUEST font SPRT OT 연결 원본 바이트 불일치"));
    }
    Ok(verify_typed_source(
        IsaProfile::DECLARED,
        SITE_RUNTIME,
        SOURCE_ID,
        &[
            K::J {
                target: entry_runtime,
            },
            delay,
        ],
    )?)
}

#[cfg(test)]
#[path = "explicit_texture_page_tests.rs"]
mod tests;
