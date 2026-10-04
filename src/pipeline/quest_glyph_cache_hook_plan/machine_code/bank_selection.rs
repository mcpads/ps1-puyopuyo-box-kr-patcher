//! 요청 page의 resident bank 선택과 pin 회피 기계어 조립.
//!
//! 공용 loader가 정규화한 page ID를 받아 primary/secondary resident 여부와
//! live object marker를 검사한다. 이 모듈은 bank 선택 내부 분기와 upload
//! 진입을 소유하고, 상위 body의 성공 복귀와 재시도 target만 fixup으로 남긴다.

use crate::disasm::{
    QUEST_FONT_OBJECT_LIST_HEAD_RUNTIME, QUEST_FONT_OBJECT_LIST_SENTINEL_RUNTIME,
    QUEST_FONT_OBJECT_NEXT_OFFSET, QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET,
    QUEST_OBJECT_POOL_RECORD_COUNT,
};
use crate::isa::{InstructionKind as K, Reg};

use super::super::super::quest_glyph_atlas_bank_policy::{
    QuestGlyphAtlasBank, QUEST_GLYPH_ATLAS_SECONDARY_MARKER_BIT,
};
use super::super::super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;
use super::super::{QuestGlyphCacheHookPlanError, QuestItemTextPageMode};
use super::{patch_branch, patch_jump, push_address};

pub(super) struct PageBankSelectionFixups {
    item_text_primary_pinned_branch: usize,
    malformed_object_list_jump: usize,
    all_banks_pinned_jump: usize,
    primary_resident_return_jump: usize,
    secondary_resident_return_jump: usize,
}

pub(super) fn append_page_bank_selection(
    kinds: &mut Vec<K>,
    item_text_pages: Option<QuestItemTextPageMode>,
    body_runtime: u32,
) -> Result<PageBankSelectionFixups, QuestGlyphCacheHookPlanError> {
    kinds.push(K::Addu {
        rd: Reg::T4,
        rs: Reg::ZERO,
        rt: Reg::ZERO,
    });
    push_address(kinds, Reg::T0, QUEST_FONT_OBJECT_LIST_HEAD_RUNTIME);
    kinds.extend([
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
    ]);
    push_address(kinds, Reg::T1, QUEST_FONT_OBJECT_LIST_SENTINEL_RUNTIME);
    kinds.push(K::Addiu {
        rt: Reg::T2,
        rs: Reg::ZERO,
        imm: QUEST_OBJECT_POOL_RECORD_COUNT as i16,
    });
    let marked_object_loop = kinds.len();
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
        K::Lbu {
            rt: Reg::T3,
            base: Reg::T0,
            offset: QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET,
        },
        K::Lw {
            rt: Reg::T0,
            base: Reg::T0,
            offset: QUEST_FONT_OBJECT_NEXT_OFFSET,
        },
        K::Beq {
            rs: Reg::T3,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Srl {
            rd: Reg::T3,
            rt: Reg::T3,
            sa: 7,
        },
        K::Addiu {
            rt: Reg::T3,
            rs: Reg::T3,
            imm: 1,
        },
        K::Or {
            rd: Reg::T4,
            rs: Reg::T4,
            rt: Reg::T3,
        },
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::T2,
            imm: -1,
        },
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
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let marked_object_done_branch = marked_object_loop;
    let unmarked_object_branch = marked_object_loop + 4;
    let marked_object_processed = marked_object_loop + 9;
    let marked_object_loop_branch = marked_object_loop + 10;
    let malformed_object_list_jump = marked_object_loop + 12;

    let bank_residency_check = kinds.len();
    push_address(kinds, Reg::T0, QUEST_PAGE_HOOK_STATE_RUNTIME);
    kinds.extend([
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: QuestGlyphAtlasBank::Primary.state_offset(),
        },
        K::Lbu {
            rt: Reg::T2,
            base: Reg::T0,
            offset: QuestGlyphAtlasBank::Secondary.state_offset(),
        },
    ]);
    let primary_state_miss_branch = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T1,
            rt: Reg::S1,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Andi {
            rt: Reg::T0,
            rs: Reg::T4,
            imm: 1,
        },
    ]);
    let primary_resident_branch = kinds.len();
    kinds.extend([
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
    let primary_stale_upload_jump = kinds.len();
    kinds.extend([
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    let secondary_residency_check = kinds.len();
    kinds.extend([
        K::Lw {
            rt: Reg::T3,
            base: Reg::SP,
            offset: 12,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    if let Some(item_pages) = item_text_pages {
        kinds.extend([
            K::Sltiu {
                rt: Reg::T0,
                rs: Reg::S1,
                imm: i16::from(item_pages.first_page_id),
            },
            K::Sltiu {
                rt: Reg::T1,
                rs: Reg::S1,
                imm: i16::from(item_pages.last_page_id) + 1,
            },
            K::Xor {
                rd: Reg::T0,
                rs: Reg::T0,
                rt: Reg::T1,
            },
            K::Or {
                rd: Reg::T3,
                rs: Reg::T3,
                rt: Reg::T0,
            },
        ]);
    }
    let item_text_skip_secondary_resident_branch = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T3,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let secondary_state_miss_branch = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T2,
            rt: Reg::S1,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Andi {
            rt: Reg::T0,
            rs: Reg::T4,
            imm: 2,
        },
    ]);
    let secondary_resident_branch = kinds.len();
    kinds.extend([
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
    let secondary_stale_upload_jump = kinds.len();
    kinds.extend([
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    let bank_availability_check = kinds.len();
    kinds.extend([
        K::Andi {
            rt: Reg::T0,
            rs: Reg::T4,
            imm: 1,
        },
        K::Beq {
            rs: Reg::T0,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lw {
            rt: Reg::T3,
            base: Reg::SP,
            offset: 12,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let primary_available_branch = bank_availability_check + 1;
    let mut item_text_replace_primary_branch = None;
    if let Some(item_pages) = item_text_pages {
        kinds.extend([
            K::Sltiu {
                rt: Reg::T0,
                rs: Reg::S1,
                imm: i16::from(item_pages.first_page_id),
            },
            K::Sltiu {
                rt: Reg::T2,
                rs: Reg::S1,
                imm: i16::from(item_pages.last_page_id) + 1,
            },
            K::Xor {
                rd: Reg::T0,
                rs: Reg::T0,
                rt: Reg::T2,
            },
        ]);
        push_address(kinds, Reg::T1, QUEST_PAGE_HOOK_STATE_RUNTIME);
        kinds.extend([
            K::Lbu {
                rt: Reg::T1,
                base: Reg::T1,
                offset: QuestGlyphAtlasBank::Primary.state_offset(),
            },
            K::Or {
                rd: Reg::T3,
                rs: Reg::T3,
                rt: Reg::T0,
            },
            // 지명의 부분 page도 item page 교체 대상이다. 위 resident 비교는
            // 상위 bit를 보존하므로 부분 page를 완성된 page로 재사용하지 않는다.
            K::Andi {
                rt: Reg::T1,
                rs: Reg::T1,
                imm: 0x7F,
            },
            K::Sltiu {
                rt: Reg::T2,
                rs: Reg::T1,
                imm: i16::from(item_pages.first_page_id),
            },
            K::Sltiu {
                rt: Reg::T1,
                rs: Reg::T1,
                imm: i16::from(item_pages.last_page_id) + 1,
            },
            K::Xor {
                rd: Reg::T1,
                rs: Reg::T2,
                rt: Reg::T1,
            },
            K::And {
                rd: Reg::T2,
                rs: Reg::T0,
                rt: Reg::T1,
            },
        ]);
        item_text_replace_primary_branch = Some(kinds.len());
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
        ]);
    }
    let item_text_primary_pinned_branch = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T3,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Andi {
            rt: Reg::T0,
            rs: Reg::T4,
            imm: 2,
        },
        K::Beq {
            rs: Reg::T0,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let secondary_available_branch = kinds.len() - 4;
    let all_banks_pinned_jump = kinds.len() - 2;

    let primary_resident = kinds.len();
    kinds.extend([
        K::Sw {
            rt: Reg::S1,
            base: Reg::SP,
            offset: 8,
        },
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let primary_resident_return_jump = primary_resident + 1;

    let secondary_resident = kinds.len();
    kinds.extend([
        K::Ori {
            rt: Reg::T0,
            rs: Reg::S1,
            imm: u16::from(QUEST_GLYPH_ATLAS_SECONDARY_MARKER_BIT),
        },
        K::Sw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 8,
        },
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let secondary_resident_return_jump = secondary_resident + 2;

    let primary_upload = kinds.len();
    kinds.extend([
        K::Sw {
            rt: Reg::S1,
            base: Reg::SP,
            offset: 8,
        },
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let primary_upload_jump = primary_upload + 1;

    let secondary_upload = kinds.len();
    kinds.extend([
        K::Ori {
            rt: Reg::T0,
            rs: Reg::S1,
            imm: u16::from(QUEST_GLYPH_ATLAS_SECONDARY_MARKER_BIT),
        },
        K::Sw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 8,
        },
    ]);
    let page_assignment_setup = kinds.len();

    for (source, target) in [
        (primary_resident_branch, primary_resident),
        (
            item_text_skip_secondary_resident_branch,
            bank_availability_check,
        ),
        (primary_state_miss_branch, secondary_residency_check),
        (secondary_state_miss_branch, bank_availability_check),
        (secondary_resident_branch, secondary_resident),
        (marked_object_done_branch, bank_residency_check),
        (unmarked_object_branch, marked_object_processed),
        (marked_object_loop_branch, marked_object_loop),
        (primary_available_branch, primary_upload),
        (secondary_available_branch, secondary_upload),
    ] {
        patch_branch(kinds, source, target)?;
    }
    if let Some(source) = item_text_replace_primary_branch {
        patch_branch(kinds, source, primary_upload)?;
    }
    for (source, target) in [
        (primary_stale_upload_jump, primary_upload),
        (secondary_stale_upload_jump, secondary_upload),
    ] {
        patch_jump(kinds, source, target, body_runtime)?;
    }
    patch_jump(
        kinds,
        primary_upload_jump,
        page_assignment_setup,
        body_runtime,
    )?;

    Ok(PageBankSelectionFixups {
        item_text_primary_pinned_branch,
        malformed_object_list_jump,
        all_banks_pinned_jump,
        primary_resident_return_jump,
        secondary_resident_return_jump,
    })
}

pub(super) fn patch_page_bank_selection_exits(
    kinds: &mut [K],
    fixups: PageBankSelectionFixups,
    body_runtime: u32,
    return_with_route_marker: usize,
    yield_without_consuming: usize,
) -> Result<(), QuestGlyphCacheHookPlanError> {
    patch_branch(
        kinds,
        fixups.item_text_primary_pinned_branch,
        yield_without_consuming,
    )?;
    for source in [
        fixups.malformed_object_list_jump,
        fixups.all_banks_pinned_jump,
    ] {
        patch_jump(kinds, source, yield_without_consuming, body_runtime)?;
    }
    for source in [
        fixups.primary_resident_return_jump,
        fixups.secondary_resident_return_jump,
    ] {
        patch_jump(kinds, source, return_with_route_marker, body_runtime)?;
    }
    Ok(())
}
