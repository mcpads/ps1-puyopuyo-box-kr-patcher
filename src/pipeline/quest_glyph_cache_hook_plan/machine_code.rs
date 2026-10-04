//! QUEST sparse glyph cache hook의 typed R3000A 기계어 생성.
//!
//! 상위 plan module이 검증한 package layout과 runtime 주소를 받아, 설치 site와
//! carrier body를 같은 ISA profile로 조립하고 연속 해석 및 load-delay 조건을
//! 검산한다.

mod bank_selection;
mod compact_return;
mod decoders;
mod font_render_routes;
mod item_object_particle;
mod item_text_routes;
mod latin_advance;
mod page_request;
mod page_upload;
mod return_routes;
mod space_width;
mod sprite_texture_page;
mod world_label_upload;

use crate::disasm::{
    QUEST_FONT_HOOK_SITE_RUNTIME, QUEST_GLYPH_RENDERER_RUNTIME, QUEST_PAGE_CARRIER_START_RUNTIME,
};
use crate::isa::{verify_typed_source, InstructionKind as K, IsaProfile, Reg, VerifiedMachineCode};

use super::super::quest_glyph_atlas_bank_policy::QUEST_GLYPH_ATLAS_LOGICAL_PAGE_MASK;
use super::super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::super::quest_glyph_cache_package::{QUEST_GLYPH_CACHE_VRAM_X, QUEST_GLYPH_CACHE_VRAM_Y};
use super::{
    error, QuestGlyphCacheHookPlanError, QuestItemTextPageMode,
    QUEST_GLYPH_CACHE_HOOK_BODY_SOURCE_ID, QUEST_GLYPH_CACHE_HOOK_SITE_SOURCE_ID,
    QUEST_GLYPH_CACHE_KOREAN_TEXTURE_PAGE, QUEST_GLYPH_CACHE_SECONDARY_TEXTURE_PAGE,
    QUEST_GLYPH_CACHE_SECONDARY_VRAM_X, QUEST_ORIGINAL_FONT_GUARD_SITE_SOURCE_ID,
};
use bank_selection::{append_page_bank_selection, patch_page_bank_selection_exits};
use decoders::{append_bounded_packbits_decoder, append_row_dictionary_decoder};
use font_render_routes::append_font_render_routes;
use item_text_routes::{append_item_request_routes, append_item_return_routes};
use page_request::{append_page_request_entry, patch_page_request_routes};
use page_upload::{append_page_upload, patch_page_upload_exits, PageUploadRuntimeSources};
use return_routes::{
    append_loader_return_routes, append_original_font_guard_return, patch_loader_return_routes,
};

#[derive(Debug, Clone, Copy)]
struct QuestGlyphCacheHookLayout {
    slot_rects_offset: usize,
    korean_descriptor_offset: usize,
    glyph_cell_indices_offset: usize,
    glyph_row_dictionary_offset: usize,
    page_directory_offset: usize,
    page_assignments_offset: usize,
    scratch_offset: usize,
}

impl From<&QuestGlyphCachePackage> for QuestGlyphCacheHookLayout {
    fn from(package: &QuestGlyphCachePackage) -> Self {
        Self {
            slot_rects_offset: package.layout.slot_rects_offset,
            korean_descriptor_offset: package.layout.korean_descriptor_offset,
            glyph_cell_indices_offset: package.layout.glyph_cell_indices_offset,
            glyph_row_dictionary_offset: package.layout.glyph_row_dictionary_offset,
            page_directory_offset: package.layout.page_directory_offset,
            page_assignments_offset: package.layout.page_assignments_offset,
            scratch_offset: package.layout.scratch_offset,
        }
    }
}

#[derive(Debug)]
pub(super) struct AssembledQuestGlyphCacheHookBody {
    pub(super) code: VerifiedMachineCode,
    pub(super) original_font_guard_entry_runtime: u32,
    pub(super) font_space_guard_entry_runtime: u32,
    pub(super) item_name_entry_runtime: Option<u32>,
    pub(super) item_name_direct_glyph_entry_runtime: Option<u32>,
    pub(super) item_name_whole_string_entry_runtime: Option<u32>,
    pub(super) item_text_mode_entry_runtime: Option<u32>,
    pub(super) item_description_constructor_entry_runtime: Option<u32>,
    pub(super) font_object_marker_entry_runtime: u32,
    pub(super) font_sprite_texture_page_entry_runtime: u32,
    pub(super) font_draw_descriptor_entry_runtime: u32,
    pub(super) font_draw_texture_page_entry_runtime: u32,
    pub(super) korean_descriptor_runtime: u32,
}

fn runtime_at(offset: usize) -> Result<u32, QuestGlyphCacheHookPlanError> {
    QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_add(
            u32::try_from(offset)
                .map_err(|_| error("QUEST glyph cache package offset이 u32를 넘음"))?,
        )
        .ok_or_else(|| error("QUEST glyph cache package runtime 주소 overflow"))
}

fn load_address(register: Reg, address: u32) -> [K; 2] {
    [
        K::Lui {
            rt: register,
            imm: (address.wrapping_add(0x8000) >> 16) as u16,
        },
        K::Addiu {
            rt: register,
            rs: register,
            imm: address as u16 as i16,
        },
    ]
}

pub(super) fn push_address(kinds: &mut Vec<K>, register: Reg, address: u32) {
    kinds.extend(load_address(register, address));
}

fn branch_offset(
    source_index: usize,
    target_index: usize,
) -> Result<i16, QuestGlyphCacheHookPlanError> {
    let next = source_index
        .checked_add(1)
        .ok_or_else(|| error("QUEST glyph cache branch source overflow"))?;
    isize::try_from(target_index)
        .ok()
        .and_then(|target| {
            isize::try_from(next)
                .ok()
                .and_then(|source| i16::try_from(target - source).ok())
        })
        .ok_or_else(|| error("QUEST glyph cache branch offset 변환 실패"))
}

pub(super) fn patch_branch(
    kinds: &mut [K],
    source_index: usize,
    target_index: usize,
) -> Result<(), QuestGlyphCacheHookPlanError> {
    let offset = branch_offset(source_index, target_index)?;
    match kinds.get_mut(source_index) {
        Some(K::Beq { offset: actual, .. })
        | Some(K::Bne { offset: actual, .. })
        | Some(K::Bgtz { offset: actual, .. }) => {
            *actual = offset;
            Ok(())
        }
        other => Err(error(format!(
            "QUEST glyph cache branch fixup 대상이 branch가 아님: index={source_index}, kind={other:?}"
        ))),
    }
}

fn body_runtime_at(body_runtime: u32, index: usize) -> Result<u32, QuestGlyphCacheHookPlanError> {
    body_runtime
        .checked_add(
            u32::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(4))
                .ok_or_else(|| error("QUEST glyph cache body index overflow"))?,
        )
        .ok_or_else(|| error("QUEST glyph cache body runtime 주소 overflow"))
}

fn patch_jump(
    kinds: &mut [K],
    source_index: usize,
    target_index: usize,
    body_runtime: u32,
) -> Result<(), QuestGlyphCacheHookPlanError> {
    let target = body_runtime_at(body_runtime, target_index)?;
    match kinds.get_mut(source_index) {
        Some(K::J { target: actual }) | Some(K::Jal { target: actual }) => {
            *actual = target;
            Ok(())
        }
        other => Err(error(format!(
            "QUEST glyph cache jump fixup 대상이 jump가 아님: index={source_index}, kind={other:?}"
        ))),
    }
}

fn is_delayed_load(kind: K) -> bool {
    matches!(
        kind,
        K::Lb { .. }
            | K::Lbu { .. }
            | K::Lh { .. }
            | K::Lhu { .. }
            | K::Lw { .. }
            | K::Lwl { .. }
            | K::Lwr { .. }
            | K::Mfc0 { .. }
            | K::Mfc2 { .. }
            | K::Cfc2 { .. }
    )
}

fn verify_load_delays(body: &VerifiedMachineCode) -> Result<(), QuestGlyphCacheHookPlanError> {
    for pair in body.instructions().windows(2) {
        let load = pair[0];
        let next = pair[1];
        if is_delayed_load(load.kind)
            && load
                .dest_reg()
                .is_some_and(|register| next.reads_reg(register))
        {
            return Err(error(format!(
                "QUEST glyph cache R3000A load delay 위반: load=0x{:08X}, next=0x{:08X}",
                load.addr, next.addr
            )));
        }
    }
    Ok(())
}

pub(super) fn assemble_site_for_runtime(
    body_runtime: u32,
) -> Result<VerifiedMachineCode, QuestGlyphCacheHookPlanError> {
    verify_typed_source(
        IsaProfile::DECLARED,
        QUEST_FONT_HOOK_SITE_RUNTIME,
        QUEST_GLYPH_CACHE_HOOK_SITE_SOURCE_ID,
        &[
            K::J {
                target: body_runtime,
            },
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ],
    )
    .map_err(Into::into)
}

pub(super) fn assemble_original_font_guard_site_for_runtime(
    guard_entry_runtime: u32,
) -> Result<VerifiedMachineCode, QuestGlyphCacheHookPlanError> {
    verify_typed_source(
        IsaProfile::DECLARED,
        QUEST_GLYPH_RENDERER_RUNTIME,
        QUEST_ORIGINAL_FONT_GUARD_SITE_SOURCE_ID,
        &[
            K::J {
                target: guard_entry_runtime,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ],
    )
    .map_err(Into::into)
}

pub(super) fn assemble_redirect_site_for_runtime(
    site_runtime: u32,
    entry_runtime: u32,
    source_id: &'static str,
) -> Result<VerifiedMachineCode, QuestGlyphCacheHookPlanError> {
    verify_typed_source(
        IsaProfile::DECLARED,
        site_runtime,
        source_id,
        &[
            K::J {
                target: entry_runtime,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ],
    )
    .map_err(Into::into)
}

fn korean_texture_page(vram_x: usize, expected: u16) -> Result<u16, QuestGlyphCacheHookPlanError> {
    let x = u16::try_from(vram_x).map_err(|_| error("QUEST Korean atlas VRAM x가 u16을 넘음"))?;
    let y = u16::try_from(QUEST_GLYPH_CACHE_VRAM_Y)
        .map_err(|_| error("QUEST Korean atlas VRAM y가 u16을 넘음"))?;
    if x > 0x03FF || y > 0x01FF || !x.is_multiple_of(64) || !y.is_multiple_of(256) {
        return Err(error("QUEST Korean atlas texture page 경계 불일치"));
    }
    let page = 0x20u16 | ((x & 0x03FF) >> 6) | ((y & 0x0100) >> 4);
    if page != expected {
        return Err(error("QUEST Korean atlas texture page 파생값 drift"));
    }
    Ok(page)
}

#[cfg(test)]
fn assemble_body_for_layout(
    layout: QuestGlyphCacheHookLayout,
    pages: usize,
    glyphs: usize,
    dictionary_rows: usize,
    body_runtime: u32,
) -> Result<AssembledQuestGlyphCacheHookBody, QuestGlyphCacheHookPlanError> {
    assemble_body_for_layout_with_item_text_pages(
        layout,
        pages,
        glyphs,
        dictionary_rows,
        body_runtime,
        None,
    )
}

#[cfg(test)]
fn assemble_body_for_layout_with_item_name_page(
    layout: QuestGlyphCacheHookLayout,
    pages: usize,
    glyphs: usize,
    dictionary_rows: usize,
    body_runtime: u32,
    item_name_page_id: Option<u8>,
) -> Result<AssembledQuestGlyphCacheHookBody, QuestGlyphCacheHookPlanError> {
    let item_text_pages = item_name_page_id.map(|page_id| QuestItemTextPageMode {
        default_name_page_id: page_id,
        first_page_id: page_id,
        last_page_id: page_id,
        shared_slots: 0,
    });
    assemble_body_for_layout_with_item_text_pages(
        layout,
        pages,
        glyphs,
        dictionary_rows,
        body_runtime,
        item_text_pages,
    )
}

#[cfg(test)]
fn assemble_body_for_layout_with_item_text_pages(
    layout: QuestGlyphCacheHookLayout,
    pages: usize,
    glyphs: usize,
    dictionary_rows: usize,
    body_runtime: u32,
    item_text_pages: Option<QuestItemTextPageMode>,
) -> Result<AssembledQuestGlyphCacheHookBody, QuestGlyphCacheHookPlanError> {
    assemble_body_with_space_slots(
        layout,
        pages,
        glyphs,
        dictionary_rows,
        body_runtime,
        item_text_pages,
        &[],
        &[],
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
fn assemble_body_with_space_slots(
    layout: QuestGlyphCacheHookLayout,
    pages: usize,
    glyphs: usize,
    dictionary_rows: usize,
    body_runtime: u32,
    item_text_pages: Option<QuestItemTextPageMode>,
    space_slots: &[Option<u16>],
    object_particle_codes: &[(u16, u16)],
    latin_advance_slots: &[Vec<(u16, i16)>],
) -> Result<AssembledQuestGlyphCacheHookBody, QuestGlyphCacheHookPlanError> {
    if !space_slots.is_empty() && space_slots.len() != pages {
        return Err(error("QUEST 공백 slot page 수 불일치"));
    }
    let item_text_routes_enabled = item_text_pages.is_some();
    let slot_rects = runtime_at(layout.slot_rects_offset)?;
    let korean_descriptor = runtime_at(layout.korean_descriptor_offset)?;
    let glyph_cell_indices = runtime_at(layout.glyph_cell_indices_offset)?;
    let glyph_row_dictionary = runtime_at(layout.glyph_row_dictionary_offset)?;
    let page_directory = runtime_at(layout.page_directory_offset)?;
    let page_assignments = runtime_at(layout.page_assignments_offset)?;
    let scratch = runtime_at(layout.scratch_offset)?;
    let primary_texture_page = korean_texture_page(
        QUEST_GLYPH_CACHE_VRAM_X,
        QUEST_GLYPH_CACHE_KOREAN_TEXTURE_PAGE,
    )?;
    let secondary_texture_page = korean_texture_page(
        QUEST_GLYPH_CACHE_SECONDARY_VRAM_X,
        QUEST_GLYPH_CACHE_SECONDARY_TEXTURE_PAGE,
    )?;
    let page_limit_exclusive = pages
        .checked_add(1)
        .and_then(|value| i16::try_from(value).ok())
        .ok_or_else(|| error("QUEST glyph cache page limit이 i16을 넘음"))?;
    if pages > usize::from(QUEST_GLYPH_ATLAS_LOGICAL_PAGE_MASK) {
        return Err(error(
            "QUEST glyph cache page가 object marker 하위 7비트를 넘음",
        ));
    }
    if item_text_pages.is_some_and(|item_pages| {
        item_pages.first_page_id == 0
            || item_pages.first_page_id > item_pages.default_name_page_id
            || item_pages.default_name_page_id > item_pages.last_page_id
            || usize::from(item_pages.last_page_id) > pages
    }) {
        return Err(error(
            "QUEST item text page 범위가 package의 유효 page 범위 밖임",
        ));
    }
    let catalog_limit_exclusive =
        i16::try_from(glyphs).map_err(|_| error("QUEST glyph cache catalog limit이 i16을 넘음"))?;
    let row_dictionary_limit_exclusive = i16::try_from(dictionary_rows)
        .map_err(|_| error("QUEST glyph cache row dictionary limit이 i16을 넘음"))?;

    let mut kinds = Vec::new();
    compact_return::append_compact_return(&mut kinds)?;
    let page_request = append_page_request_entry(&mut kinds, page_limit_exclusive);
    let bank_selection = append_page_bank_selection(&mut kinds, item_text_pages, body_runtime)?;

    let page_upload = append_page_upload(
        &mut kinds,
        PageUploadRuntimeSources {
            slot_rects,
            glyph_cell_indices,
            glyph_row_dictionary,
            page_directory,
            page_assignments,
            scratch,
        },
        catalog_limit_exclusive,
        body_runtime,
    )?;

    let loader_returns = append_loader_return_routes(&mut kinds);
    let item_returns =
        append_item_return_routes(&mut kinds, item_text_routes_enabled, object_particle_codes)?;

    let original_font_guard_return = append_original_font_guard_return(&mut kinds);
    let row_decoder =
        append_row_dictionary_decoder(&mut kinds, row_dictionary_limit_exclusive, body_runtime)?;
    append_bounded_packbits_decoder(&mut kinds, body_runtime)?;

    let font_render_routes = append_font_render_routes(
        &mut kinds,
        korean_descriptor,
        primary_texture_page,
        secondary_texture_page,
        item_text_routes_enabled,
        space_slots,
        latin_advance_slots,
    )?;

    let sprite_texture_page = sprite_texture_page::append_sprite_texture_page(&mut kinds)?;
    let item_text_routes = append_item_request_routes(
        &mut kinds,
        item_returns,
        item_text_pages,
        page_request.entry(),
        original_font_guard_return,
        body_runtime,
    )?;

    patch_loader_return_routes(
        &mut kinds,
        loader_returns,
        item_text_routes.successful_non_op21_return,
        item_text_routes.failed_non_op21_return,
    )?;
    patch_page_request_routes(&mut kinds, page_request, loader_returns.successful_return())?;
    patch_page_bank_selection_exits(
        &mut kinds,
        bank_selection,
        body_runtime,
        loader_returns.successful_return(),
        loader_returns.retry_return(),
    )?;
    patch_page_upload_exits(
        &mut kinds,
        page_upload,
        body_runtime,
        row_decoder,
        loader_returns.retry_return(),
    )?;
    let code = verify_typed_source(
        IsaProfile::DECLARED,
        body_runtime,
        QUEST_GLYPH_CACHE_HOOK_BODY_SOURCE_ID,
        &kinds,
    )?;
    verify_load_delays(&code)?;
    Ok(AssembledQuestGlyphCacheHookBody {
        code,
        original_font_guard_entry_runtime: body_runtime_at(
            body_runtime,
            font_render_routes.original_font_guard,
        )?,
        font_space_guard_entry_runtime: body_runtime_at(
            body_runtime,
            font_render_routes.space_guard,
        )?,
        item_name_entry_runtime: item_text_routes
            .callback
            .map(|entry| body_runtime_at(body_runtime, entry))
            .transpose()?,
        item_name_direct_glyph_entry_runtime: item_text_routes
            .direct_glyph
            .map(|entry| body_runtime_at(body_runtime, entry))
            .transpose()?,
        item_name_whole_string_entry_runtime: item_text_routes
            .whole_string
            .map(|entry| body_runtime_at(body_runtime, entry))
            .transpose()?,
        item_text_mode_entry_runtime: item_text_routes
            .text_mode
            .map(|entry| body_runtime_at(body_runtime, entry))
            .transpose()?,
        item_description_constructor_entry_runtime: item_text_routes
            .description_constructor
            .map(|entry| body_runtime_at(body_runtime, entry))
            .transpose()?,
        font_object_marker_entry_runtime: body_runtime_at(
            body_runtime,
            font_render_routes.object_marker,
        )?,
        font_draw_descriptor_entry_runtime: body_runtime_at(
            body_runtime,
            font_render_routes.draw_descriptor,
        )?,
        font_draw_texture_page_entry_runtime: body_runtime_at(
            body_runtime,
            font_render_routes.draw_texture_page,
        )?,
        font_sprite_texture_page_entry_runtime: body_runtime_at(body_runtime, sprite_texture_page)?,
        korean_descriptor_runtime: korean_descriptor,
    })
}

pub(super) fn assemble_body_for_package(
    package: &QuestGlyphCachePackage,
    body_runtime: u32,
) -> Result<AssembledQuestGlyphCacheHookBody, QuestGlyphCacheHookPlanError> {
    assemble_body_for_package_with_item_text_pages(package, body_runtime, None)
}

pub(super) fn assemble_body_for_package_with_item_text_pages(
    package: &QuestGlyphCachePackage,
    body_runtime: u32,
    mut item_text_pages: Option<QuestItemTextPageMode>,
) -> Result<AssembledQuestGlyphCacheHookBody, QuestGlyphCacheHookPlanError> {
    // 모든 item page에서 같은 code를 쓰는 음절만 선택한다. 이름과 조사 자체의
    // 공급 완전성은 item text writer에서 원문 결속 이름별로 재검산한다.
    let mut particle_codes = Vec::new();
    if let Some(mode) = &mut item_text_pages {
        let pages = (mode.first_page_id..=mode.last_page_id)
            .map(|id| {
                let indices = package
                    .page_catalog_indices(id)
                    .map_err(|e| error(e.to_string()))?;
                // 메뉴용 alias는 catalog ID가 달라도 Denki cell이 같을 수 있다.
                // 실제 package를 해제한 픽셀까지 같을 때만 공통 slot으로 취급한다.
                indices
                    .into_iter()
                    .map(|index| package.glyph_cell(index).map_err(|e| error(e.to_string())))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let first = pages.first().ok_or_else(|| error("item page가 비었음"))?;
        let shared = first
            .iter()
            .enumerate()
            .take_while(|(slot, glyph)| pages.iter().all(|page| page.get(*slot) == Some(*glyph)))
            .count();
        mode.shared_slots = u16::try_from(shared).map_err(|_| error("공통 item slot 수 초과"))?;
    }
    if let Some(mode) = item_text_pages {
        let pages = package
            .object_particle_codes
            .get(usize::from(mode.first_page_id - 1)..usize::from(mode.last_page_id))
            .ok_or_else(|| error("QUEST 목적격 조사 page 범위 누락"))?;
        if let Some(first) = pages.first() {
            particle_codes = first
                .iter()
                .copied()
                .filter(|pair| pages.iter().all(|page| page.contains(pair)))
                .collect();
        }
    }
    assemble_body_with_space_slots(
        package.into(),
        package.plan.pages,
        package.plan.glyphs,
        package.plan.catalog.dictionary_rows,
        body_runtime,
        item_text_pages,
        &package.space_slots,
        &particle_codes,
        &package.latin_advance_slots,
    )
}

#[cfg(test)]
#[path = "machine_code_tests.rs"]
mod tests;
