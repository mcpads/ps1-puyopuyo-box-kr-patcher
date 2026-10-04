//! ITEM_DAT 설명 constructor 앞에서 page residency를 보장하는 typed site.
//!
//! 세 목록 callback의 무입력 경로는 매 순회 constructor를 다시 부른다.
//! 호출을 공용 residency entry로 옮기면 page 준비가 끝난 순회에서만 원본
//! constructor로 tail-call하고, 재시도 순회에서는 원본 caller로 돌아갈 수 있다.

use super::quest_glyph_atlas_bank_policy::QuestGlyphAtlasBank;
use super::quest_item_glyph_cache_dispatch::{
    push_address, push_item_entry_frame, QuestItemTextModeEntry,
    QUEST_ITEM_DESCRIPTION_MODE_CONSTRUCTOR,
};
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;
use crate::disasm::{
    QuestItemDescriptionSelectionPath, QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
    QUEST_ITEM_DESCRIPTION_POINTER_FIELD_OFFSET, QUEST_ITEM_RECORD_TABLE_RUNTIME,
};
use crate::isa::{
    verify_typed_source, InstructionKind as K, IsaProfile, Reg, VerifiedMachineCode, VerifyError,
};
use crate::script::{BOX_QUEST_ITEM_RECORDS, QUEST_SEQ_TEXT_MODE_OPCODE};

pub(super) const QUEST_ITEM_DESCRIPTION_RESIDENCY_PATHS: usize = 3;
pub(super) const QUEST_ITEM_DESCRIPTION_RESIDENCY_SITE_SOURCE_ID: &str =
    "pipeline::quest_item_description_residency_hook::constructor_site";

pub(super) fn assemble_item_description_residency_sites(
    paths: &[QuestItemDescriptionSelectionPath; QUEST_ITEM_DESCRIPTION_RESIDENCY_PATHS],
    entry_runtime: u32,
) -> Result<Vec<VerifiedMachineCode>, VerifyError> {
    paths
        .iter()
        .map(|path| {
            verify_typed_source(
                IsaProfile::DECLARED,
                path.constructor_call_runtime,
                QUEST_ITEM_DESCRIPTION_RESIDENCY_SITE_SOURCE_ID,
                &[
                    K::Jal {
                        target: entry_runtime,
                    },
                    K::Andi {
                        rt: Reg::A0,
                        rs: Reg::V0,
                        imm: 0x00FF,
                    },
                ],
            )
        })
        .collect()
}

pub(super) fn append_item_description_constructor_entry(
    kinds: &mut Vec<K>,
    first_item_text_page_id: u8,
    last_item_text_page_id: u8,
) -> QuestItemTextModeEntry {
    let entry = kinds.len();
    let item_record_limit =
        i16::try_from(BOX_QUEST_ITEM_RECORDS).expect("ITEM_DAT record 수는 i16 범위 안임");
    kinds.extend([
        K::Andi {
            rt: Reg::T0,
            rs: Reg::A0,
            imm: 0x00FF,
        },
        K::Sltiu {
            rt: Reg::T2,
            rs: Reg::T0,
            imm: item_record_limit,
        },
        K::Beq {
            rs: Reg::T2,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Sll {
            rd: Reg::T0,
            rt: Reg::T0,
            sa: 2,
        },
    ]);
    let invalid_item_id_branch = entry + 2;
    push_address(kinds, Reg::T1, QUEST_ITEM_RECORD_TABLE_RUNTIME);
    kinds.extend([
        K::Addu {
            rd: Reg::T0,
            rs: Reg::T1,
            rt: Reg::T0,
        },
        K::Lw {
            rt: Reg::T0,
            base: Reg::T0,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lw {
            rt: Reg::T0,
            base: Reg::T0,
            offset: QUEST_ITEM_DESCRIPTION_POINTER_FIELD_OFFSET,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: -4,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_SEQ_TEXT_MODE_OPCODE),
        },
    ]);
    let opcode_mismatch_branch = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T1,
            rt: Reg::T2,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: -2,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let metadata_byte_two_mismatch_branch = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T1,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: -1,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let metadata_byte_three_mismatch_branch = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T1,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: -3,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Sltiu {
            rt: Reg::T2,
            rs: Reg::T1,
            imm: i16::from(first_item_text_page_id),
        },
    ]);
    let page_below_range_branch = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T2,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Sltiu {
            rt: Reg::T2,
            rs: Reg::T1,
            imm: i16::from(last_item_text_page_id) + 1,
        },
    ]);
    let page_above_range_branch = kinds.len();
    kinds.extend([
        K::Beq {
            rs: Reg::T2,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let mut bypass_branches = vec![
        invalid_item_id_branch,
        opcode_mismatch_branch,
        metadata_byte_two_mismatch_branch,
        metadata_byte_three_mismatch_branch,
        page_below_range_branch,
        page_above_range_branch,
    ];
    push_address(kinds, Reg::T2, QUEST_PAGE_HOOK_STATE_RUNTIME);
    kinds.extend([
        K::Lbu {
            rt: Reg::T2,
            base: Reg::T2,
            offset: QuestGlyphAtlasBank::Primary.state_offset(),
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Beq {
            rs: Reg::T2,
            rt: Reg::T1,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    bypass_branches.push(kinds.len() - 2);
    push_item_entry_frame(kinds, QUEST_ITEM_DESCRIPTION_MODE_CONSTRUCTOR);
    kinds.push(K::Addu {
        rd: Reg::S1,
        rs: Reg::T1,
        rt: Reg::ZERO,
    });
    kinds.extend([
        K::J { target: 0 },
        K::Addu {
            rd: Reg::S2,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
    ]);
    let page_request_jump = kinds.len() - 2;
    let bypass = kinds.len();
    kinds.extend([
        K::J {
            target: QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    QuestItemTextModeEntry {
        entry,
        page_request_jump,
        bypass_branches,
        bypass,
    }
}

#[cfg(test)]
#[path = "quest_item_description_residency_hook_tests.rs"]
mod tests;
