//! QUEST glyph cache의 font object와 renderer 소비 경로 조립.
//!
//! object marker, descriptor, texture page 선택은 cache page 공급과 별개인
//! 소비자 경로다. 이 모듈은 네 entry와 그 내부 branch patch를 함께 소유한다.

use crate::disasm::{
    QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME, QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME,
    QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME, QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET,
    QUEST_FONT_OBJECT_SCALE_X_OFFSET, QUEST_FONT_OBJECT_STATE_INDEX_OFFSET,
    QUEST_GLYPH_RENDERER_CONTINUATION_RUNTIME, QUEST_SEQ_TEXT_GLYPH_RENDERER_RETURN_RUNTIME,
};
use crate::isa::{InstructionKind as K, Reg};

use super::super::super::quest_glyph_atlas_bank_policy::QUEST_GLYPH_ATLAS_SECONDARY_MARKER_BIT;
use super::super::super::quest_item_name_glyph_cache_hook::append_item_name_renderer_filter;
use super::super::{
    QuestGlyphCacheHookPlanError, QUEST_GLYPH_CACHE_RENDERER_PAGE_RUNTIME,
    QUEST_GLYPH_CACHE_ROUTE_PAGE_RUNTIME,
};
use super::{patch_branch, push_address};

// FF1E의 같은 callback을 쓰는 다른 stream은 원본 경로로 둔다.
fn append_encounter_name_filter(
    kinds: &mut Vec<K>,
) -> Result<(usize, usize), QuestGlyphCacheHookPlanError> {
    push_address(
        kinds,
        Reg::T1,
        crate::analysis::QUEST_ENCOUNTER_NAME_GLYPH_RETURN_RUNTIME,
    );
    let other_caller = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::RA,
            rt: Reg::T1,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lw {
            rt: Reg::T2,
            base: Reg::S4,
            offset: 0x1C,
        },
    ]);
    push_address(
        kinds,
        Reg::T1,
        crate::analysis::QUEST_ENCOUNTER_NAME_STREAM_CURSOR_RUNTIME,
    );
    let other_stream = kinds.len();
    kinds.extend([
        K::Bne {
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
    let selected = kinds.len();
    kinds.extend([
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
    let next_filter = kinds.len();
    patch_branch(kinds, other_caller, next_filter)?;
    Ok((selected, other_stream))
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FontRenderRouteEntries {
    pub(super) original_font_guard: usize,
    pub(super) space_guard: usize,
    pub(super) object_marker: usize,
    pub(super) draw_descriptor: usize,
    pub(super) draw_texture_page: usize,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_font_render_routes(
    kinds: &mut Vec<K>,
    korean_descriptor: u32,
    primary_texture_page: u16,
    secondary_texture_page: u16,
    item_name_route_enabled: bool,
    space_slots: &[Option<u16>],
    latin_advance_slots: &[Vec<(u16, i16)>],
) -> Result<FontRenderRouteEntries, QuestGlyphCacheHookPlanError> {
    let original_font_guard = kinds.len();
    push_address(kinds, Reg::T0, QUEST_GLYPH_CACHE_RENDERER_PAGE_RUNTIME);
    kinds.push(K::Sb {
        rt: Reg::ZERO,
        base: Reg::T0,
        offset: 0,
    });
    let encounter_filter = if item_name_route_enabled {
        Some(append_encounter_name_filter(kinds)?)
    } else {
        None
    };
    push_address(kinds, Reg::T1, QUEST_SEQ_TEXT_GLYPH_RENDERER_RETURN_RUNTIME);
    let item_name_renderer_filter =
        append_item_name_renderer_filter(kinds, item_name_route_enabled);
    push_address(kinds, Reg::T1, QUEST_GLYPH_CACHE_ROUTE_PAGE_RUNTIME);
    kinds.extend([
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T1,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Sb {
            rt: Reg::T1,
            base: Reg::T0,
            offset: 0,
        },
    ]);
    let replay_original_renderer = kinds.len();
    kinds.extend([
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -32,
        },
        K::J {
            target: QUEST_GLYPH_RENDERER_CONTINUATION_RUNTIME,
        },
        K::Sw {
            rt: Reg::S0,
            base: Reg::SP,
            offset: 16,
        },
    ]);
    for source in item_name_renderer_filter.selected_branches {
        patch_branch(kinds, source, item_name_renderer_filter.selected)?;
    }
    for source in item_name_renderer_filter.unselected_branches {
        patch_branch(kinds, source, replay_original_renderer)?;
    }
    if let Some((selected, other_stream)) = encounter_filter {
        patch_branch(kinds, selected, item_name_renderer_filter.selected)?;
        patch_branch(kinds, other_stream, replay_original_renderer)?;
    }

    let object_marker = kinds.len();
    kinds.push(K::Lbu {
        rt: Reg::V0,
        base: Reg::V1,
        offset: 0x0E,
    });
    push_address(kinds, Reg::T0, QUEST_GLYPH_CACHE_RENDERER_PAGE_RUNTIME);
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
        K::Sb {
            rt: Reg::T0,
            base: Reg::A2,
            offset: QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET,
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
        // FF10·FF08이 만든 가로 효과를 유지한다. 공백만 그 배율의 반폭이다.
        K::Lw {
            rt: Reg::T1,
            base: Reg::A2,
            offset: QUEST_FONT_OBJECT_SCALE_X_OFFSET,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    super::space_width::append_space_width(kinds, space_slots)?;
    super::latin_advance::append_latin_advance(kinds, latin_advance_slots)?;
    kinds.extend([
        K::Sw {
            rt: Reg::T1,
            base: Reg::A2,
            offset: QUEST_FONT_OBJECT_SCALE_X_OFFSET,
        },
        K::J {
            target: QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME + 8,
        },
        K::Sb {
            rt: Reg::S0,
            base: Reg::A2,
            offset: QUEST_FONT_OBJECT_STATE_INDEX_OFFSET,
        },
    ]);
    let original_object_scale_branch = object_marker + 6;
    let finish_object_marker = kinds.len() - 2;
    patch_branch(kinds, original_object_scale_branch, finish_object_marker)?;

    let draw_descriptor = kinds.len();
    kinds.extend([
        K::Lbu {
            rt: Reg::T0,
            base: Reg::S1,
            offset: QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Beq {
            rs: Reg::T0,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::A0,
        },
    ]);
    let original_descriptor_branch = draw_descriptor + 2;
    push_address(kinds, Reg::A3, korean_descriptor);
    kinds.extend([
        K::J {
            target: QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME + 8,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let original_descriptor = kinds.len();
    kinds.extend([
        K::Lw {
            rt: Reg::A3,
            base: Reg::V1,
            offset: 0x10,
        },
        K::J {
            target: QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME + 8,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    patch_branch(kinds, original_descriptor_branch, original_descriptor)?;

    let draw_texture_page = kinds.len();
    kinds.extend([
        K::Lbu {
            rt: Reg::T0,
            base: Reg::S1,
            offset: QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Beq {
            rs: Reg::T0,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Addu {
            rd: Reg::A0,
            rs: Reg::A0,
            rt: Reg::T5,
        },
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: primary_texture_page as i16,
        },
        K::Andi {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: u16::from(QUEST_GLYPH_ATLAS_SECONDARY_MARKER_BIT),
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
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: secondary_texture_page as i16,
        },
    ]);
    let primary_texture_page_branch = kinds.len() - 3;
    let finish_texture_page = kinds.len();
    kinds.extend([
        K::J {
            target: QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME + 8,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let original_texture_page_branch = draw_texture_page + 2;
    let original_texture_page = kinds.len();
    kinds.extend([
        K::Lhu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0x38,
        },
        K::J {
            target: QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME + 8,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    patch_branch(kinds, primary_texture_page_branch, finish_texture_page)?;
    patch_branch(kinds, original_texture_page_branch, original_texture_page)?;

    let space_guard = super::super::space_guard::append(kinds, space_slots)?;
    Ok(FontRenderRouteEntries {
        original_font_guard,
        space_guard,
        object_marker,
        draw_descriptor,
        draw_texture_page,
    })
}

#[cfg(test)]
#[path = "font_render_routes_tests.rs"]
mod tests;
