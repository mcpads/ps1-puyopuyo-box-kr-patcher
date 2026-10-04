//! QUEST 지명·공용 선택지를 item page wrapper로 보내는 typed site.
//!
//! 공용 direct/whole-string entry와 page 요청 ABI는 item text hook이 소유한다.
//! 이 모듈은 네 선택 callback의 여덟 glyph call과 world-map 지명 draw call의
//! 원래 delay slot만 보존해 해당 entry로 우회한다.

use crate::disasm::{
    QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES, QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME,
};
use crate::isa::{
    verify_typed_source, InstructionKind as K, IsaProfile, Reg, VerifiedMachineCode, VerifyError,
};

pub(super) const QUEST_UI_CHOICE_DIRECT_GLYPH_SITE_SOURCE_ID: &str =
    "pipeline::quest_ui_glyph_cache_hook::choice_direct_glyph_site";
pub(super) const QUEST_WORLD_MAP_LABEL_WHOLE_STRING_SITE_SOURCE_ID: &str =
    "pipeline::quest_ui_glyph_cache_hook::world_map_whole_string_site";

pub(super) fn assemble_quest_ui_choice_direct_glyph_sites(
    direct_glyph_entry_runtime: u32,
) -> Result<Vec<VerifiedMachineCode>, VerifyError> {
    QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES
        .into_iter()
        .map(|runtime| {
            verify_typed_source(
                IsaProfile::DECLARED,
                runtime,
                QUEST_UI_CHOICE_DIRECT_GLYPH_SITE_SOURCE_ID,
                &[
                    K::Jal {
                        target: direct_glyph_entry_runtime,
                    },
                    K::Addiu {
                        rt: Reg::S0,
                        rs: Reg::S0,
                        imm: 2,
                    },
                ],
            )
        })
        .collect()
}

pub(super) fn assemble_world_map_label_whole_string_site(
    whole_string_entry_runtime: u32,
) -> Result<VerifiedMachineCode, VerifyError> {
    verify_typed_source(
        IsaProfile::DECLARED,
        QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME,
        QUEST_WORLD_MAP_LABEL_WHOLE_STRING_SITE_SOURCE_ID,
        &[
            K::Jal {
                target: whole_string_entry_runtime,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ],
    )
}

/// 상태·장비·도구의 direct/whole-string call과 원본 delay slot을 함께 보존한다.
pub(super) fn assemble_menu_literal_sites(
    direct: u32,
    whole: u32,
) -> Result<Vec<VerifiedMachineCode>, VerifyError> {
    crate::disasm::QUEST_MENU_GLYPH_CALLS
        .iter()
        .map(|call| {
            verify_typed_source(
                IsaProfile::DECLARED,
                call.runtime,
                "pipeline::quest_ui_glyph_cache_hook::menu_literal_site",
                &[
                    K::Jal {
                        target: if call.whole_string { whole } else { direct },
                    },
                    call.delay,
                ],
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "quest_ui_glyph_cache_hook_tests.rs"]
mod tests;
