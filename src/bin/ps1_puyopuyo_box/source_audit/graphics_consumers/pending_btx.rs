//! pending BTX 모집단, static cell과 새 일본어 양성 surface를 검증한다.

use crate::cli::BoxExecutable;

use ps1_puyopuyo_box::disasm::{BtxDescriptorSelectionAnalysis, BtxNameAnalysis};
use ps1_puyopuyo_box::graphics::text_candidates::BOX_PENDING_TEXT_ASSETS;
use ps1_puyopuyo_box::pipeline::{
    audit_btx_pending_consumer_plan, verify_box_btx_pending_consumer_plan,
    verify_box_btx_pending_static_cells, verify_box_btx_text_surface_cells,
    verify_puyo1_ending_asset_map, BtxPendingStaticCellAnalysis,
};
use ps1_puyopuyo_box::script::SubgameSeqLayoutAnalysis;

pub(super) fn audit_pending_btx(
    box_executables: &[BoxExecutable],
    subgame_seq_sources: &[(String, String, Vec<u8>)],
    subgame_seq_layouts: &SubgameSeqLayoutAnalysis,
    btx_name_consumers: &BtxNameAnalysis,
    pending_btx_selections: &BtxDescriptorSelectionAnalysis,
) -> Result<BtxPendingStaticCellAnalysis, String> {
    println!(
        "PASS pending BTX descriptor selections: EXE/GRP boundaries {}, \
         resource x36 {}, cache guards {}, copy36 {}, exact +4 read PCs {}, entry-reached {}",
        pending_btx_selections.selections.len(),
        pending_btx_selections.selections.len(),
        pending_btx_selections.selections.len(),
        pending_btx_selections.selections.len(),
        pending_btx_selections.selections.len(),
        pending_btx_selections
            .selections
            .iter()
            .filter(|selection| selection.entry_reached)
            .count(),
    );
    let puyo1_ending_assets = verify_puyo1_ending_asset_map(&btx_name_consumers.links)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS PUYO1 ending BTX resource map: descriptor root 0x{:08X}, \
         runtime-observed source bindings {} ({:?}), protected PUYO2 copies {} ({:?}); \
         runtime consumption is separate evidence",
        puyo1_ending_assets.descriptor_runtime_start,
        puyo1_ending_assets.runtime_selected_assets.len(),
        puyo1_ending_assets
            .runtime_selected_assets
            .iter()
            .map(|binding| binding.resource)
            .collect::<Vec<_>>(),
        puyo1_ending_assets.protected_puyo2_copies.len(),
        puyo1_ending_assets
            .protected_puyo2_copies
            .iter()
            .map(|binding| binding.resource)
            .collect::<Vec<_>>(),
    );
    let pending_btx_consumers =
        verify_box_btx_pending_consumer_plan(btx_name_consumers, pending_btx_selections)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS pending BTX consumer plan: findings {}, physical rows {}, EXE/GRP boundaries {}, \
         representative {}, physical-copy followup {}, runtime unverified",
        pending_btx_consumers.findings,
        pending_btx_consumers.rows.len(),
        pending_btx_consumers.execution_boundaries,
        pending_btx_consumers.representative_boundaries,
        pending_btx_consumers.physical_copy_followup_boundaries,
    );
    let current_pending_consumers = audit_btx_pending_consumer_plan(
        &BOX_PENDING_TEXT_ASSETS,
        btx_name_consumers,
        pending_btx_selections,
    )
    .map_err(|error| error.to_string())?;
    if current_pending_consumers.findings != 3
        || current_pending_consumers.rows.len() != 4
        || current_pending_consumers.execution_boundaries != 2
        || current_pending_consumers.representative_boundaries != 2
        || current_pending_consumers.physical_copy_followup_boundaries != 0
    {
        return Err(format!(
            "current pending BTX consumer 분모 불일치: {current_pending_consumers:?}"
        ));
    }
    println!(
        "PASS current pending BTX consumer plan: findings {}, physical rows {}, \
         EXE/GRP boundaries {}, representative {}, physical-copy followup {}; runtime unverified",
        current_pending_consumers.findings,
        current_pending_consumers.rows.len(),
        current_pending_consumers.execution_boundaries,
        current_pending_consumers.representative_boundaries,
        current_pending_consumers.physical_copy_followup_boundaries,
    );
    let pending_btx_static_cells = verify_box_btx_pending_static_cells(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        subgame_seq_layouts,
        btx_name_consumers,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS pending BTX static cell supply: findings {}, physical {}, setup records {}, \
         rows with setup {}, review-layout matches {}, plain-layout matches {}; \
         execution/draw/screen unverified",
        pending_btx_static_cells.findings,
        pending_btx_static_cells.physical_assets,
        pending_btx_static_cells.setup_records,
        pending_btx_static_cells.rows_with_setup,
        pending_btx_static_cells.layout_matching_setup_records,
        pending_btx_static_cells.plain_layout_matching_setup_records,
    );
    let text_surface_cells = verify_box_btx_text_surface_cells(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        subgame_seq_layouts,
        btx_name_consumers,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS new Japanese-positive PP1/PP2 BTX static cells: findings {}, physical {}, \
         setup records {}, exact layout/plain matches {}/{}; PP1 ending setup conflicts and \
         MM_MES incomplete routes preserved; P1_OVER3 text cells 12 + protected non-cell tail; \
         P2_END10 credit cells 17, P2_END15 cells background/background/mixed, \
         P2_END16 text/text/tower/tower cells 4; \
         execution/draw/screen unverified",
        text_surface_cells.findings,
        text_surface_cells.physical_assets,
        text_surface_cells.setup_records,
        text_surface_cells.layout_matching_setup_records,
        text_surface_cells.plain_layout_matching_setup_records,
    );

    Ok(text_surface_cells)
}
