//! 선택된 glyph page의 assignment 해석·cell upload·상태 commit 조립.
//!
//! 이 모듈은 page directory와 assignment encoding을 읽어 cell을 해제하고,
//! bank별 VRAM rect에 upload한 뒤 draw sync 이후에만 resident state를 commit한다.
//! 상위 body에는 decoder entry와 실패 재시도 target fixup만 남긴다.

use crate::disasm::{QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME, QUEST_FONT_UPLOAD_TARGET_RUNTIME};
use crate::isa::{InstructionKind as K, Reg};

use super::super::super::quest_glyph_atlas_bank_policy::{
    QuestGlyphAtlasBank, QUEST_GLYPH_ATLAS_SECONDARY_MARKER_BIT,
};
use super::super::super::quest_glyph_cache_package::QUEST_GLYPH_CACHE_VRAM_X;
use super::super::super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;
use super::super::{
    QuestGlyphCacheHookPlanError, QUEST_GLYPH_CACHE_ASSIGNMENT_MODE_STACK_OFFSET,
    QUEST_GLYPH_CACHE_SECONDARY_VRAM_X, QUEST_GLYPH_CACHE_UPLOAD_ROWS,
    QUEST_GLYPH_CACHE_UPLOAD_WORD_WIDTH,
};
use super::{patch_branch, patch_jump, push_address};

#[derive(Debug, Clone, Copy)]
pub(super) struct PageUploadRuntimeSources {
    pub(super) slot_rects: u32,
    pub(super) glyph_cell_indices: u32,
    pub(super) glyph_row_dictionary: u32,
    pub(super) page_directory: u32,
    pub(super) page_assignments: u32,
    pub(super) scratch: u32,
}

pub(super) struct PageUploadFixups {
    decoder_call: usize,
    failure_branches: [usize; 6],
}

pub(super) fn append_page_upload(
    kinds: &mut Vec<K>,
    sources: PageUploadRuntimeSources,
    catalog_limit_exclusive: i16,
    body_runtime: u32,
) -> Result<PageUploadFixups, QuestGlyphCacheHookPlanError> {
    let PageUploadRuntimeSources {
        slot_rects,
        glyph_cell_indices,
        glyph_row_dictionary,
        page_directory,
        page_assignments,
        scratch,
    } = sources;
    super::world_label_upload::append_selection(kinds)?;
    kinds.extend([
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::S1,
            imm: -1,
        },
        K::Sll {
            rd: Reg::T0,
            rt: Reg::T0,
            sa: 1,
        },
    ]);
    push_address(kinds, Reg::T1, page_directory);
    kinds.extend([
        K::Addu {
            rd: Reg::T0,
            rs: Reg::T1,
            rt: Reg::T0,
        },
        K::Lhu {
            rt: Reg::S2,
            base: Reg::T0,
            offset: 0,
        },
        K::Lhu {
            rt: Reg::S3,
            base: Reg::T0,
            offset: 2,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    push_address(kinds, Reg::T1, page_assignments);
    kinds.extend([
        K::Addu {
            rd: Reg::S2,
            rs: Reg::T1,
            rt: Reg::S2,
        },
        K::Addu {
            rd: Reg::S3,
            rs: Reg::T1,
            rt: Reg::S3,
        },
    ]);
    let assignment_encoding_setup = kinds.len();
    kinds.extend([
        K::Beq {
            rs: Reg::S2,
            rt: Reg::S3,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lbu {
            rt: Reg::T0,
            base: Reg::S2,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: 0x0080,
        },
        K::Bne {
            rs: Reg::T0,
            rt: Reg::T1,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::S2,
            imm: 2,
        },
        K::Sltu {
            rd: Reg::T1,
            rs: Reg::S3,
            rt: Reg::T0,
        },
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
            rt: Reg::T0,
            base: Reg::S2,
            offset: 1,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
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
        K::Addiu {
            rt: Reg::S2,
            rs: Reg::S2,
            imm: 2,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::ZERO,
            imm: 1,
        },
        K::Sw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: QUEST_GLYPH_CACHE_ASSIGNMENT_MODE_STACK_OFFSET,
        },
    ]);
    let assignment_empty_branch = assignment_encoding_setup;
    let assignment_delta_branch = assignment_encoding_setup + 4;
    let assignment_short_delta_branch = assignment_encoding_setup + 8;
    let assignment_nonzero_second_delta_branch = assignment_encoding_setup + 12;
    let assignment_decoder_ready = kinds.len();
    push_address(kinds, Reg::S6, glyph_cell_indices);
    push_address(kinds, Reg::S7, glyph_row_dictionary);
    kinds.extend([
        K::Addu {
            rd: Reg::S4,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        K::Addu {
            rd: Reg::S5,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
    ]);
    let assignment_loop = kinds.len();
    kinds.extend([
        K::Beq {
            rs: Reg::S2,
            rt: Reg::S3,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: QUEST_GLYPH_CACHE_ASSIGNMENT_MODE_STACK_OFFSET,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
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
    let absolute_assignment_branch = assignment_loop + 4;
    let delta_assignment = kinds.len();
    kinds.extend([
        K::Lbu {
            rt: Reg::T0,
            base: Reg::S2,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::S2,
            rs: Reg::S2,
            imm: 1,
        },
        K::Andi {
            rt: Reg::T1,
            rs: Reg::T0,
            imm: 0x007F,
        },
        K::Andi {
            rt: Reg::T2,
            rs: Reg::T0,
            imm: 0x0080,
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
        K::Beq {
            rs: Reg::S2,
            rt: Reg::S3,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lbu {
            rt: Reg::T0,
            base: Reg::S2,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::S2,
            rs: Reg::S2,
            imm: 1,
        },
        K::Andi {
            rt: Reg::T2,
            rs: Reg::T0,
            imm: 0x0080,
        },
        K::Bne {
            rs: Reg::T2,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Andi {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 0x007F,
        },
        K::Sll {
            rd: Reg::T0,
            rt: Reg::T0,
            sa: 7,
        },
        K::Or {
            rd: Reg::T1,
            rs: Reg::T1,
            rt: Reg::T0,
        },
        K::Bne {
            rs: Reg::T1,
            rt: Reg::ZERO,
            offset: 5,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Beq {
            rs: Reg::S5,
            rt: Reg::ZERO,
            offset: 3,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::J { target: 0 },
        K::Addiu {
            rt: Reg::S4,
            rs: Reg::ZERO,
            imm: -1,
        },
        K::Addu {
            rd: Reg::S4,
            rs: Reg::S4,
            rt: Reg::T1,
        },
        K::Sltiu {
            rt: Reg::T0,
            rs: Reg::S4,
            imm: catalog_limit_exclusive,
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
    let assignment_done_branch = assignment_loop;
    let delta_ready_branch = delta_assignment + 4;
    let delta_truncated_branch = delta_assignment + 6;
    let delta_overlong_branch = delta_assignment + 11;
    let catalog_range_branch = delta_assignment + 23;
    let process_jump = delta_assignment + 25;

    let delta_restart_jump = delta_assignment + 19;

    let absolute_assignment = kinds.len();
    kinds.extend([
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::S2,
            imm: 2,
        },
        K::Sltu {
            rd: Reg::T1,
            rs: Reg::S3,
            rt: Reg::T0,
        },
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
            rt: Reg::S4,
            base: Reg::S2,
            offset: 0,
        },
        K::Lbu {
            rt: Reg::T0,
            base: Reg::S2,
            offset: 1,
        },
        K::Addiu {
            rt: Reg::S2,
            rs: Reg::S2,
            imm: 2,
        },
        K::Sll {
            rd: Reg::T0,
            rt: Reg::T0,
            sa: 8,
        },
        K::Or {
            rd: Reg::S4,
            rs: Reg::S4,
            rt: Reg::T0,
        },
        K::Sltiu {
            rt: Reg::T0,
            rs: Reg::S4,
            imm: catalog_limit_exclusive,
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
    let absolute_truncated_branch = absolute_assignment + 2;
    let absolute_catalog_range_branch = absolute_assignment + 10;
    let absolute_process_jump = absolute_assignment + 12;

    let process_cell = kinds.len();
    let directory_bytes = catalog_limit_exclusive
        .checked_add(1)
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| super::error("QUEST glyph cell directory가 addiu 범위를 넘음"))?;
    let unused_cell_branch = super::world_label_upload::append_filter(kinds)?;
    kinds.extend([
        K::Sll {
            rd: Reg::T0,
            rt: Reg::S4,
            sa: 1,
        },
        K::Addu {
            rd: Reg::T0,
            rs: Reg::S6,
            rt: Reg::T0,
        },
        K::Lhu {
            rt: Reg::A0,
            base: Reg::T0,
            offset: 0,
        },
        K::Lhu {
            rt: Reg::A3,
            base: Reg::T0,
            offset: 2,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::S6,
            imm: directory_bytes,
        },
        K::Addu {
            rd: Reg::A0,
            rs: Reg::T0,
            rt: Reg::A0,
        },
        K::Addu {
            rd: Reg::A3,
            rs: Reg::T0,
            rt: Reg::A3,
        },
        K::Addu {
            rd: Reg::A1,
            rs: Reg::S7,
            rt: Reg::ZERO,
        },
    ]);
    push_address(kinds, Reg::A2, scratch);
    let glyph_decoder_call = kinds.len();
    kinds.extend([
        K::Jal { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let glyph_decode_done_jump = kinds.len();
    kinds.extend([
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let decode_result = kinds.len();
    kinds.extend([
        K::Beq {
            rs: Reg::V0,
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
            rt: Reg::S5,
            sa: 2,
        },
    ]);
    let decoder_failure_branch = decode_result;
    push_address(kinds, Reg::T1, slot_rects);
    kinds.extend([
        K::Addu {
            rd: Reg::T0,
            rs: Reg::T1,
            rt: Reg::T0,
        },
        K::Lw {
            rt: Reg::T1,
            base: Reg::T0,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lw {
            rt: Reg::T2,
            base: Reg::SP,
            offset: 8,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Andi {
            rt: Reg::T2,
            rs: Reg::T2,
            imm: u16::from(QUEST_GLYPH_ATLAS_SECONDARY_MARKER_BIT),
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
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::T1,
            imm: (QUEST_GLYPH_CACHE_SECONDARY_VRAM_X - QUEST_GLYPH_CACHE_VRAM_X) as i16,
        },
    ]);
    let primary_upload_rect_branch = kinds.len() - 3;
    let upload_rect_ready = kinds.len();
    kinds.extend([
        K::Sw {
            rt: Reg::T1,
            base: Reg::SP,
            offset: 0,
        },
        K::Lui {
            rt: Reg::T1,
            imm: QUEST_GLYPH_CACHE_UPLOAD_ROWS,
        },
        K::Ori {
            rt: Reg::T1,
            rs: Reg::T1,
            imm: QUEST_GLYPH_CACHE_UPLOAD_WORD_WIDTH,
        },
        K::Sw {
            rt: Reg::T1,
            base: Reg::SP,
            offset: 4,
        },
        K::Addu {
            rd: Reg::A0,
            rs: Reg::SP,
            rt: Reg::ZERO,
        },
    ]);
    push_address(kinds, Reg::A1, scratch);
    kinds.extend([
        K::Jal {
            target: QUEST_FONT_UPLOAD_TARGET_RUNTIME,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Jal {
            target: QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
        },
        K::Addu {
            rd: Reg::A0,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        K::Addiu {
            rt: Reg::S5,
            rs: Reg::S5,
            imm: 1,
        },
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let assignment_loop_jump = kinds.len() - 2;
    let next_cell = assignment_loop_jump - 1;

    let upload_done = kinds.len();
    super::world_label_upload::append_partial_commit(kinds);
    let commit_start = kinds.len();
    push_address(kinds, Reg::T0, QUEST_PAGE_HOOK_STATE_RUNTIME);
    kinds.extend([
        K::Lw {
            rt: Reg::T1,
            base: Reg::SP,
            offset: 8,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Andi {
            rt: Reg::T2,
            rs: Reg::T1,
            imm: u16::from(QUEST_GLYPH_ATLAS_SECONDARY_MARKER_BIT),
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
        K::Sb {
            rt: Reg::S1,
            base: Reg::T0,
            offset: QuestGlyphAtlasBank::Primary.state_offset(),
        },
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let commit_secondary_branch = commit_start + 5;
    let commit_done_jump = commit_start + 8;
    let commit_secondary = kinds.len();
    kinds.push(K::Sb {
        rt: Reg::S1,
        base: Reg::T0,
        offset: QuestGlyphAtlasBank::Secondary.state_offset(),
    });
    let commit_done = kinds.len();

    for (source, target) in [
        (assignment_empty_branch, assignment_loop),
        (assignment_delta_branch, assignment_decoder_ready),
        (assignment_short_delta_branch, assignment_decoder_ready),
        (
            assignment_nonzero_second_delta_branch,
            assignment_decoder_ready,
        ),
        (assignment_done_branch, upload_done),
        (unused_cell_branch, next_cell),
        (absolute_assignment_branch, absolute_assignment),
        (delta_ready_branch, delta_assignment + 15),
        (primary_upload_rect_branch, upload_rect_ready),
        (commit_secondary_branch, commit_secondary),
    ] {
        patch_branch(kinds, source, target)?;
    }
    for (source, target) in [
        (delta_restart_jump, assignment_loop),
        (process_jump, process_cell),
        (absolute_process_jump, process_cell),
        (glyph_decode_done_jump, decode_result),
        (assignment_loop_jump, assignment_loop),
        (commit_done_jump, commit_done),
    ] {
        patch_jump(kinds, source, target, body_runtime)?;
    }

    Ok(PageUploadFixups {
        decoder_call: glyph_decoder_call,
        failure_branches: [
            delta_truncated_branch,
            delta_overlong_branch,
            catalog_range_branch,
            absolute_truncated_branch,
            absolute_catalog_range_branch,
            decoder_failure_branch,
        ],
    })
}

pub(super) fn patch_page_upload_exits(
    kinds: &mut [K],
    fixups: PageUploadFixups,
    body_runtime: u32,
    row_decoder: usize,
    yield_without_consuming: usize,
) -> Result<(), QuestGlyphCacheHookPlanError> {
    for source in fixups.failure_branches {
        patch_branch(kinds, source, yield_without_consuming)?;
    }
    patch_jump(kinds, fixups.decoder_call, row_decoder, body_runtime)?;
    Ok(())
}
