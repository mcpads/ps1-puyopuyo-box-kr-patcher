//! ITEM_DAT page 요청이 공유하는 frame ABI와 성공·재시도 dispatcher.
//!
//! 이름 callback, direct glyph, whole-string, 설명 direct text와 constructor는
//! 같은 page loader를 쓰지만 복귀 target은 서로 다르다. 이 모듈은 mode별
//! 복귀 경로와 저장 frame만 소유하고, 각 소비자의 metadata 검증과 설치 site는
//! 이름·설명 전용 모듈에 남긴다.

use crate::disasm::{
    QUEST_GLYPH_RENDERER_RUNTIME, QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
    QUEST_ITEM_NAME_CALLBACK_EPILOGUE_RUNTIME, QUEST_ITEM_NAME_RESIDENCY_CONTINUATION_RUNTIME,
    QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME, QUEST_SEQ_TEXT_CONSUMER_RUNTIME,
};
use crate::isa::{InstructionKind as K, Reg};

pub const QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME: u32 =
    super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME - 1;

pub(super) const QUEST_ITEM_NAME_MODE_CALLBACK: i16 = 1;
pub(super) const QUEST_ITEM_NAME_MODE_DIRECT_GLYPH: i16 = 2;
pub(super) const QUEST_ITEM_NAME_MODE_WHOLE_STRING: i16 = 3;
pub(super) const QUEST_ITEM_TEXT_MODE_RESIDENCY: i16 = 4;
pub(super) const QUEST_ITEM_DESCRIPTION_MODE_CONSTRUCTOR: i16 = 5;

#[derive(Default)]
pub(super) struct QuestItemReturnPaths {
    pub(super) callback_success: Option<usize>,
    pub(super) callback_retry: Option<usize>,
    pub(super) direct_success: Option<usize>,
    pub(super) direct_retry: Option<usize>,
    pub(super) whole_string_success: Option<usize>,
    pub(super) whole_string_retry: Option<usize>,
    pub(super) text_mode_success: Option<usize>,
    pub(super) text_mode_retry: Option<usize>,
    pub(super) description_constructor_success: Option<usize>,
    pub(super) description_constructor_retry: Option<usize>,
    pub(super) success_direct_branch: Option<usize>,
    pub(super) success_whole_string_branch: Option<usize>,
    pub(super) success_text_mode_branch: Option<usize>,
    pub(super) success_description_constructor_branch: Option<usize>,
    pub(super) success_callback_branch: Option<usize>,
    pub(super) retry_direct_branch: Option<usize>,
    pub(super) retry_whole_string_branch: Option<usize>,
    pub(super) retry_text_mode_branch: Option<usize>,
    pub(super) retry_description_constructor_branch: Option<usize>,
    pub(super) retry_callback_branch: Option<usize>,
}

pub(super) struct QuestItemTextModeEntry {
    pub(super) entry: usize,
    pub(super) page_request_jump: usize,
    pub(super) bypass_branches: Vec<usize>,
    pub(super) bypass: usize,
}

pub(super) fn push_address(kinds: &mut Vec<K>, register: Reg, address: u32) {
    kinds.extend([
        K::Lui {
            rt: register,
            imm: (address.wrapping_add(0x8000) >> 16) as u16,
        },
        K::Addiu {
            rt: register,
            rs: register,
            imm: address as u16 as i16,
        },
    ]);
}

pub(super) fn push_restore_frame(kinds: &mut Vec<K>) {
    kinds.extend([
        K::Lw {
            rt: Reg::A0,
            base: Reg::SP,
            offset: 56,
        },
        K::Lw {
            rt: Reg::A1,
            base: Reg::SP,
            offset: 20,
        },
        K::Lw {
            rt: Reg::A2,
            base: Reg::SP,
            offset: 16,
        },
        K::Lw {
            rt: Reg::RA,
            base: Reg::SP,
            offset: 60,
        },
        K::Lw {
            rt: Reg::S0,
            base: Reg::SP,
            offset: 52,
        },
        K::Lw {
            rt: Reg::S1,
            base: Reg::SP,
            offset: 48,
        },
        K::Lw {
            rt: Reg::S2,
            base: Reg::SP,
            offset: 44,
        },
        K::Lw {
            rt: Reg::S3,
            base: Reg::SP,
            offset: 40,
        },
        K::Lw {
            rt: Reg::S4,
            base: Reg::SP,
            offset: 36,
        },
        K::Lw {
            rt: Reg::S5,
            base: Reg::SP,
            offset: 32,
        },
        K::Lw {
            rt: Reg::S6,
            base: Reg::SP,
            offset: 28,
        },
        K::Lw {
            rt: Reg::S7,
            base: Reg::SP,
            offset: 24,
        },
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: super::quest_glyph_cache_hook_plan::QUEST_GLYPH_CACHE_FRAME_BYTES,
        },
    ]);
}

pub(super) fn append_item_return_paths(
    kinds: &mut Vec<K>,
    enabled: bool,
    particle: &[K],
    direct_particle: &[K],
) -> QuestItemReturnPaths {
    if !enabled {
        return QuestItemReturnPaths::default();
    }

    let callback_success = kinds.len();
    push_restore_frame(kinds);
    kinds.extend([
        K::J {
            target: QUEST_ITEM_NAME_RESIDENCY_CONTINUATION_RUNTIME,
        },
        K::Addu {
            rd: Reg::S2,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
    ]);

    let callback_retry = kinds.len();
    push_restore_frame(kinds);
    kinds.extend([
        K::J {
            target: QUEST_ITEM_NAME_CALLBACK_EPILOGUE_RUNTIME,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    let direct_success = kinds.len();
    kinds.extend_from_slice(direct_particle);
    push_restore_frame(kinds);
    kinds.extend([
        K::J {
            target: QUEST_GLYPH_RENDERER_RUNTIME,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    let direct_retry = kinds.len();
    push_restore_frame(kinds);
    kinds.extend([
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    let whole_string_success = kinds.len();
    kinds.extend([
        K::Lw {
            rt: Reg::A0,
            base: Reg::SP,
            offset: 56,
        },
        K::Lw {
            rt: Reg::A1,
            base: Reg::SP,
            offset: 20,
        },
        K::Lw {
            rt: Reg::A2,
            base: Reg::SP,
            offset: 16,
        },
    ]);
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
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    kinds.extend_from_slice(particle);
    push_address(kinds, Reg::T0, QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME);
    kinds.push(K::Sb {
        rt: Reg::ZERO,
        base: Reg::T0,
        offset: 0,
    });
    push_restore_frame(kinds);
    kinds.extend([
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    let whole_string_retry = kinds.len();
    push_restore_frame(kinds);
    kinds.extend([
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    let text_mode_success = kinds.len();
    push_restore_frame(kinds);
    kinds.extend([
        K::J {
            target: QUEST_SEQ_TEXT_CONSUMER_RUNTIME,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    let text_mode_retry = kinds.len();
    push_restore_frame(kinds);
    kinds.extend([
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    let description_constructor_success = kinds.len();
    push_restore_frame(kinds);
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

    let description_constructor_retry = kinds.len();
    push_restore_frame(kinds);
    kinds.extend([
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    let success_dispatch = kinds.len();
    kinds.extend([
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: QUEST_ITEM_NAME_MODE_DIRECT_GLYPH,
        },
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
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: QUEST_ITEM_NAME_MODE_WHOLE_STRING,
        },
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
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: QUEST_ITEM_TEXT_MODE_RESIDENCY,
        },
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
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: QUEST_ITEM_DESCRIPTION_MODE_CONSTRUCTOR,
        },
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
        K::Beq {
            rs: Reg::ZERO,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    let retry_dispatch = kinds.len();
    kinds.extend([
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: QUEST_ITEM_NAME_MODE_DIRECT_GLYPH,
        },
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
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: QUEST_ITEM_NAME_MODE_WHOLE_STRING,
        },
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
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: QUEST_ITEM_TEXT_MODE_RESIDENCY,
        },
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
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: QUEST_ITEM_DESCRIPTION_MODE_CONSTRUCTOR,
        },
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
        K::Beq {
            rs: Reg::ZERO,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    QuestItemReturnPaths {
        callback_success: Some(callback_success),
        callback_retry: Some(callback_retry),
        direct_success: Some(direct_success),
        direct_retry: Some(direct_retry),
        whole_string_success: Some(whole_string_success),
        whole_string_retry: Some(whole_string_retry),
        text_mode_success: Some(text_mode_success),
        text_mode_retry: Some(text_mode_retry),
        description_constructor_success: Some(description_constructor_success),
        description_constructor_retry: Some(description_constructor_retry),
        success_direct_branch: Some(success_dispatch + 1),
        success_whole_string_branch: Some(success_dispatch + 4),
        success_text_mode_branch: Some(success_dispatch + 7),
        success_description_constructor_branch: Some(success_dispatch + 10),
        success_callback_branch: Some(success_dispatch + 12),
        retry_direct_branch: Some(retry_dispatch + 1),
        retry_whole_string_branch: Some(retry_dispatch + 4),
        retry_text_mode_branch: Some(retry_dispatch + 7),
        retry_description_constructor_branch: Some(retry_dispatch + 10),
        retry_callback_branch: Some(retry_dispatch + 12),
    }
}

pub(super) fn push_item_entry_frame(kinds: &mut Vec<K>, mode: i16) {
    kinds.extend([
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -super::quest_glyph_cache_hook_plan::QUEST_GLYPH_CACHE_FRAME_BYTES,
        },
        K::Sw {
            rt: Reg::RA,
            base: Reg::SP,
            offset: 60,
        },
        K::Sw {
            rt: Reg::A0,
            base: Reg::SP,
            offset: 56,
        },
        K::Sw {
            rt: Reg::S0,
            base: Reg::SP,
            offset: 52,
        },
        K::Sw {
            rt: Reg::S1,
            base: Reg::SP,
            offset: 48,
        },
        K::Sw {
            rt: Reg::S2,
            base: Reg::SP,
            offset: 44,
        },
        K::Sw {
            rt: Reg::S3,
            base: Reg::SP,
            offset: 40,
        },
        K::Sw {
            rt: Reg::S4,
            base: Reg::SP,
            offset: 36,
        },
        K::Sw {
            rt: Reg::S5,
            base: Reg::SP,
            offset: 32,
        },
        K::Sw {
            rt: Reg::S6,
            base: Reg::SP,
            offset: 28,
        },
        K::Sw {
            rt: Reg::S7,
            base: Reg::SP,
            offset: 24,
        },
        K::Sw {
            rt: Reg::A1,
            base: Reg::SP,
            offset: 20,
        },
        K::Sw {
            rt: Reg::A2,
            base: Reg::SP,
            offset: 16,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::ZERO,
            imm: mode,
        },
        K::Sw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 12,
        },
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::SP,
            offset: 8,
        },
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::SP,
            offset:
                super::quest_glyph_cache_hook_plan::QUEST_GLYPH_CACHE_ASSIGNMENT_MODE_STACK_OFFSET,
        },
    ]);
}
