//! 아이템 획득 고정 문자열 draw를 item page 래퍼로 보내는 typed site.
//!
//! 공용 whole-string entry와 page 요청 ABI는 이름 hook이 소유한다. 이
//! 모듈은 `OPAA` 상태기의 여섯 고정 문자열 call과 각 delay slot만 보존한다.

use crate::disasm::QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES;
use crate::isa::{
    verify_typed_source, InstructionKind as K, IsaProfile, Reg, VerifiedMachineCode, VerifyError,
};

pub(super) const QUEST_ITEM_ACQUISITION_WHOLE_STRING_SITE_SOURCE_ID: &str =
    "pipeline::quest_item_acquisition_glyph_cache_hook::whole_string_site";

fn delay_instruction(runtime: u32) -> K {
    match runtime {
        0x8006_14E4 => K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        0x8006_17B8 => K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 0x2234,
        },
        0x8006_17FC => K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 0x225C,
        },
        0x8006_18E8 => K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 0x2270,
        },
        0x8006_1904 => K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: -0x1E10,
        },
        0x8006_191C => K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 0x227C,
        },
        _ => unreachable!("고정된 OPAA draw call 분모"),
    }
}

pub(super) fn assemble_item_acquisition_whole_string_sites(
    whole_string_entry_runtime: u32,
) -> Result<Vec<VerifiedMachineCode>, VerifyError> {
    QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES
        .into_iter()
        .map(|runtime| {
            verify_typed_source(
                IsaProfile::DECLARED,
                runtime,
                QUEST_ITEM_ACQUISITION_WHOLE_STRING_SITE_SOURCE_ID,
                &[
                    K::Jal {
                        target: whole_string_entry_runtime,
                    },
                    delay_instruction(runtime),
                ],
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "quest_item_acquisition_glyph_cache_hook_tests.rs"]
mod tests;
