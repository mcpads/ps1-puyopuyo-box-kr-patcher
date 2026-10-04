//! exact PoC image에 귀속된 main RAM·GPU dump를 정적 build 결과와 대조한다.

use crate::coords::{exe_map, psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::{
    verify_box_quest_hook_space_candidates, QUEST_FONT_STATE_BASE_RUNTIME,
    QUEST_FONT_STATE_RECORD_COUNT, QUEST_FONT_STATE_RECORD_STRIDE,
};
use crate::disc::{Iso9660, PsxExe, VerifiedSourceImage};

use super::super::quest_hook_poc::{verify_quest_hook_noop_sources, QUEST_HOOK_BODY_RUNTIME};
use super::super::quest_poc::{prepare_quest_poc_assets, QuestPocMode};
use super::super::QUEST_EXE_ISO_PATH;
use super::machine_code::{assemble_body, assemble_transition_body};
use super::{
    build_quest_page_first_dialogue, build_quest_page_transition, digest, error,
    QuestPageFirstDialogueError, QuestPageFirstDialogueRuntimeReport, QuestPageTransitionPhase,
    QuestPageTransitionRuntimeReport, PSX_GPU_RAM_BYTES, PSX_GPU_ROW_BYTES,
    QUEST_FONT_STATE_ACTIVE_OFFSET, QUEST_FONT_VRAM_ROWS, QUEST_FONT_VRAM_ROW_BYTES,
    QUEST_FONT_VRAM_X_BYTES, QUEST_PAGE_CARRIER_START_RUNTIME, QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID,
    QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME, QUEST_PAGE_FIRST_DIALOGUE_RUNTIME_SCHEMA_VERSION,
    QUEST_PAGE_HOOK_STATE_RUNTIME, QUEST_PAGE_RESTORE_SOURCE_RUNTIME,
    QUEST_PAGE_TRANSITION_RUNTIME_SCHEMA_VERSION,
};

fn runtime_slice<'a>(
    ram: &'a [u8],
    runtime: u32,
    len: usize,
    label: &str,
) -> Result<&'a [u8], QuestPageFirstDialogueError> {
    let start = psx_main_ram_offset(runtime, len)
        .ok_or_else(|| error(format!("{label}: PS1 main RAM 범위 이탈")))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| error(format!("{label}: RAM 범위 overflow")))?;
    ram.get(start..end)
        .ok_or_else(|| error(format!("{label}: RAM dump 범위 이탈")))
}

fn require_runtime_exact(
    actual: &[u8],
    expected: &[u8],
    label: &str,
) -> Result<(), QuestPageFirstDialogueError> {
    if actual != expected {
        return Err(error(format!(
            "{label} 불일치: expected_sha256={}, actual_sha256={}",
            digest(expected),
            digest(actual)
        )));
    }
    Ok(())
}

pub(super) fn uploaded_font_page(gpu: &[u8]) -> Result<Vec<u8>, QuestPageFirstDialogueError> {
    if gpu.len() != PSX_GPU_RAM_BYTES {
        return Err(error(format!(
            "PS1 GPU RAM dump 길이 불일치: expected={PSX_GPU_RAM_BYTES}, actual={}",
            gpu.len()
        )));
    }
    let mut page = Vec::with_capacity(QUEST_FONT_VRAM_ROW_BYTES * QUEST_FONT_VRAM_ROWS);
    for row in 0..QUEST_FONT_VRAM_ROWS {
        let start = row
            .checked_mul(PSX_GPU_ROW_BYTES)
            .and_then(|value| value.checked_add(QUEST_FONT_VRAM_X_BYTES))
            .ok_or_else(|| error("QUEST font VRAM row offset overflow"))?;
        let end = start
            .checked_add(QUEST_FONT_VRAM_ROW_BYTES)
            .ok_or_else(|| error("QUEST font VRAM row end overflow"))?;
        page.extend_from_slice(
            gpu.get(start..end)
                .ok_or_else(|| error("QUEST font VRAM rectangle 범위 이탈"))?,
        );
    }
    Ok(page)
}

/// 첫 승인 대사 commit 뒤 main RAM과 VRAM을 exact PoC 재빌드 결과와 대조한다.
pub fn verify_quest_page_first_dialogue_runtime(
    source: &VerifiedSourceImage,
    ram: &[u8],
    gpu: &[u8],
) -> Result<QuestPageFirstDialogueRuntimeReport, QuestPageFirstDialogueError> {
    if ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "PS1 main RAM dump 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            ram.len()
        )));
    }

    let prepared = prepare_quest_poc_assets(source, QuestPocMode::Korean)
        .map_err(|source| error(source.to_string()))?;
    let build = build_quest_page_first_dialogue(source)?;
    let uploaded_page = uploaded_font_page(gpu)?;
    require_runtime_exact(
        &uploaded_page,
        &prepared.new_font_plain,
        "QUEST uploaded font page",
    )?;

    let source_iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let original_quest_raw = source_iso
        .read_file(QUEST_EXE_ISO_PATH)
        .map_err(|source| error(source.to_string()))?;
    let original_exe =
        PsxExe::parse(&original_quest_raw).map_err(|source| error(source.to_string()))?;
    let map = exe_map("QUEST.EXE").ok_or_else(|| error("QUEST.EXE 좌표 사상이 없음"))?;
    original_exe
        .verify_map(map)
        .map_err(|source| error(source.to_string()))?;
    let candidates = verify_box_quest_hook_space_candidates(map, original_exe.text())
        .map_err(|source| error(source.to_string()))?;
    let sources = verify_quest_hook_noop_sources(map, original_exe.text(), &candidates)
        .map_err(|source| error(source.to_string()))?;
    let body = assemble_body()?;

    let loaded_site = runtime_slice(
        ram,
        crate::disasm::QUEST_FONT_HOOK_SITE_RUNTIME,
        sources.site.bytes().len(),
        "QUEST 첫 대사 hook site",
    )?;
    require_runtime_exact(loaded_site, sources.site.bytes(), "QUEST 첫 대사 hook site")?;
    let loaded_body = runtime_slice(
        ram,
        QUEST_HOOK_BODY_RUNTIME,
        body.bytes().len(),
        "QUEST 첫 대사 hook body",
    )?;
    require_runtime_exact(loaded_body, body.bytes(), "QUEST 첫 대사 hook body")?;

    let resident_page = runtime_slice(
        ram,
        QUEST_PAGE_CARRIER_START_RUNTIME,
        prepared.new_font_plain.len(),
        "QUEST resident font page",
    )?;
    require_runtime_exact(
        resident_page,
        &prepared.new_font_plain,
        "QUEST resident font page",
    )?;
    let font_states_inactive = (0..QUEST_FONT_STATE_RECORD_COUNT).all(|index| {
        let runtime = QUEST_FONT_STATE_BASE_RUNTIME
            + index as u32 * QUEST_FONT_STATE_RECORD_STRIDE as u32
            + QUEST_FONT_STATE_ACTIVE_OFFSET as u32;
        runtime_slice(ram, runtime, 1, "QUEST font active selector").is_ok_and(|value| value == [0])
    });
    if !font_states_inactive {
        return Err(error("QUEST font active selector 10개 중 nonzero가 있음"));
    }
    let page_state = runtime_slice(ram, QUEST_PAGE_HOOK_STATE_RUNTIME, 1, "QUEST page state")?[0];
    let page_state_committed = page_state == QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID;
    if !page_state_committed {
        return Err(error(format!(
            "QUEST page state commit 불일치: expected={}, actual={page_state}",
            QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID
        )));
    }

    Ok(QuestPageFirstDialogueRuntimeReport {
        schema_version: QUEST_PAGE_FIRST_DIALOGUE_RUNTIME_SCHEMA_VERSION,
        image_sha256: build.report.image_sha256,
        ram_sha256: digest(ram),
        gpu_sha256: digest(gpu),
        target_record_runtime: QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME,
        page_source_runtime: QUEST_PAGE_CARRIER_START_RUNTIME,
        page_bytes: resident_page.len(),
        page_sha256: digest(resident_page),
        uploaded_page_sha256: digest(&uploaded_page),
        hook_site_exact: true,
        hook_body_exact: true,
        resident_page_exact: true,
        uploaded_page_exact: true,
        font_states_inactive,
        page_state,
        page_state_committed,
        runtime_verified: true,
        release_eligible: false,
    })
}

/// exact two-page 빌드의 한 checkpoint에서 resident source와 VRAM을 검증한다.
pub fn verify_quest_page_transition_runtime(
    source: &VerifiedSourceImage,
    ram: &[u8],
    gpu: &[u8],
    phase: QuestPageTransitionPhase,
) -> Result<QuestPageTransitionRuntimeReport, QuestPageFirstDialogueError> {
    if ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "PS1 main RAM dump 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            ram.len()
        )));
    }

    let prepared = prepare_quest_poc_assets(source, QuestPocMode::Korean)
        .map_err(|source| error(source.to_string()))?;
    let build = build_quest_page_transition(source)?;
    let uploaded_page = uploaded_font_page(gpu)?;
    let expected_uploaded = match phase {
        QuestPageTransitionPhase::Korean => &prepared.new_font_plain,
        QuestPageTransitionPhase::Restored => &prepared.original_font_plain,
    };
    require_runtime_exact(
        &uploaded_page,
        expected_uploaded,
        "QUEST transition uploaded font page",
    )?;

    let source_iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let original_quest_raw = source_iso
        .read_file(QUEST_EXE_ISO_PATH)
        .map_err(|source| error(source.to_string()))?;
    let original_exe =
        PsxExe::parse(&original_quest_raw).map_err(|source| error(source.to_string()))?;
    let map = exe_map("QUEST.EXE").ok_or_else(|| error("QUEST.EXE 좌표 사상이 없음"))?;
    original_exe
        .verify_map(map)
        .map_err(|source| error(source.to_string()))?;
    let candidates = verify_box_quest_hook_space_candidates(map, original_exe.text())
        .map_err(|source| error(source.to_string()))?;
    let sources = verify_quest_hook_noop_sources(map, original_exe.text(), &candidates)
        .map_err(|source| error(source.to_string()))?;
    let body = assemble_transition_body()?;

    let loaded_site = runtime_slice(
        ram,
        crate::disasm::QUEST_FONT_HOOK_SITE_RUNTIME,
        sources.site.bytes().len(),
        "QUEST transition hook site",
    )?;
    require_runtime_exact(
        loaded_site,
        sources.site.bytes(),
        "QUEST transition hook site",
    )?;
    let loaded_body = runtime_slice(
        ram,
        QUEST_HOOK_BODY_RUNTIME,
        body.bytes().len(),
        "QUEST transition hook body",
    )?;
    require_runtime_exact(loaded_body, body.bytes(), "QUEST transition hook body")?;

    let korean_page = runtime_slice(
        ram,
        QUEST_PAGE_CARRIER_START_RUNTIME,
        prepared.new_font_plain.len(),
        "QUEST resident Korean font page",
    )?;
    require_runtime_exact(
        korean_page,
        &prepared.new_font_plain,
        "QUEST resident Korean font page",
    )?;
    let original_page = runtime_slice(
        ram,
        QUEST_PAGE_RESTORE_SOURCE_RUNTIME,
        prepared.original_font_plain.len(),
        "QUEST resident original font page",
    )?;
    require_runtime_exact(
        original_page,
        &prepared.original_font_plain,
        "QUEST resident original font page",
    )?;
    let font_states_inactive = (0..QUEST_FONT_STATE_RECORD_COUNT).all(|index| {
        let runtime = QUEST_FONT_STATE_BASE_RUNTIME
            + index as u32 * QUEST_FONT_STATE_RECORD_STRIDE as u32
            + QUEST_FONT_STATE_ACTIVE_OFFSET as u32;
        runtime_slice(ram, runtime, 1, "QUEST font active selector").is_ok_and(|value| value == [0])
    });
    if !font_states_inactive {
        return Err(error("QUEST font active selector 10개 중 nonzero가 있음"));
    }
    let page_state = runtime_slice(ram, QUEST_PAGE_HOOK_STATE_RUNTIME, 1, "QUEST page state")?[0];
    let page_state_committed = page_state == phase.page_id();
    if !page_state_committed {
        return Err(error(format!(
            "QUEST transition page state 불일치: phase={}, expected={}, actual={page_state}",
            phase.label(),
            phase.page_id()
        )));
    }

    Ok(QuestPageTransitionRuntimeReport {
        schema_version: QUEST_PAGE_TRANSITION_RUNTIME_SCHEMA_VERSION,
        phase,
        image_sha256: build.report.image_sha256,
        ram_sha256: digest(ram),
        gpu_sha256: digest(gpu),
        expected_page_source_runtime: phase.source_runtime(),
        page_bytes: expected_uploaded.len(),
        korean_page_sha256: digest(korean_page),
        original_page_sha256: digest(original_page),
        uploaded_page_sha256: digest(&uploaded_page),
        hook_site_exact: true,
        hook_body_exact: true,
        resident_pages_exact: true,
        uploaded_page_exact: true,
        font_states_inactive,
        page_state,
        page_state_committed,
        runtime_verified: true,
        release_eligible: false,
    })
}
