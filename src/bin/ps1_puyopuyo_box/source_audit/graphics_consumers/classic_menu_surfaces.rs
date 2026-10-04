//! Classic menu static cell과 draw route를 검증한다.

use crate::cli::BoxExecutable;

use ps1_puyopuyo_box::disasm::{
    BtxDescriptorSelectionAnalysis, BtxNameAnalysis, CommonFontDrawSiteAnalysis,
};
use ps1_puyopuyo_box::pipeline::{
    verify_box_menu_static_cells, verify_box_p2_info2_static_cells,
    verify_box_p2ruled1_static_cells, verify_puyo1_menu01_draw_route,
    verify_puyo2_menu201_draw_route, verify_puyo2_menu202_draw_route,
    verify_puyo2_menu203_draw_route, verify_puyo2_p2ruled1_draw_route, ClassicMenuDrawRoute,
};
use ps1_puyopuyo_box::script::SubgameSeqLayoutAnalysis;

fn print_classic_ui_draw_route(route_name: &str, route: &ClassicMenuDrawRoute) {
    println!(
        "PASS {route_name} draw route: resource {}, descriptor/read 0x{:08X}/0x{:08X}, \
         setup/table 0x{:08X}/0x{:08X}, selector {} -> record {} at 0x{:08X} \
         bbox ({},{}) {}x{} = {}:{:?}, renderer lookup/primitive 0x{:08X}/0x{:08X}; \
         runtime screen and VRAM evidence separate",
        route.resource_id,
        route.descriptor_runtime,
        route.descriptor_cell_table_read_runtime,
        route.setup_record_runtime,
        route.cell_table_runtime,
        route.cell_selector,
        route.cell_record_index,
        route.cell_record_runtime,
        route.cell_record.x,
        route.cell_record.y,
        route.cell_record.width,
        route.cell_record.height,
        route.text_block_id,
        route.source_text,
        route.cell_lookup_runtime,
        route.primitive_runtime,
    );
}

pub(super) fn audit_classic_menu_surfaces(
    box_executables: &[BoxExecutable],
    subgame_seq_sources: &[(String, String, Vec<u8>)],
    subgame_seq_layouts: &SubgameSeqLayoutAnalysis,
    btx_name_consumers: &BtxNameAnalysis,
    pending_btx_selections: &BtxDescriptorSelectionAnalysis,
    common_font_draw_sites: &CommonFontDrawSiteAnalysis,
) -> Result<(), String> {
    let p2_info2_static_cells = verify_box_p2_info2_static_cells(
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
        "PASS P2_INFO2 static sprite cells: physical {}, setup records {}, exact 144x176 \
         layout matches {}, inferred valid cells 26; PUYO1 256x176 setups conflict; \
         execution/draw/screen unverified",
        p2_info2_static_cells.physical_assets,
        p2_info2_static_cells.setup_records,
        p2_info2_static_cells.layout_matching_setup_records,
    );
    let p2ruled1_static_cells = verify_box_p2ruled1_static_cells(
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
        "PASS P2RULED1 static cells: physical {}, setup records {}, exact 256x176 \
         layout/plain matches {}/{}; representative runtime evidence is checked by its draw route",
        p2ruled1_static_cells.physical_assets,
        p2ruled1_static_cells.setup_records,
        p2ruled1_static_cells.layout_matching_setup_records,
        p2ruled1_static_cells.plain_layout_matching_setup_records,
    );
    let menu_static_cells = verify_box_menu_static_cells(
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
        "PASS PUYO1/PUYO2 menu static cells: findings {}, physical {}, setup records {}, \
         exact layout/plain matches {}/{}; MENU01/06/201/202/203 Japanese-positive, \
         representative runtime evidence is checked by separate draw routes",
        menu_static_cells.findings,
        menu_static_cells.physical_assets,
        menu_static_cells.setup_records,
        menu_static_cells.layout_matching_setup_records,
        menu_static_cells.plain_layout_matching_setup_records,
    );
    let puyo2_menu201_draw = verify_puyo2_menu201_draw_route(
        &btx_name_consumers.links,
        pending_btx_selections,
        &menu_static_cells,
        common_font_draw_sites,
    )
    .map_err(|error| error.to_string())?;
    print_classic_ui_draw_route("PUYO2 MENU201", &puyo2_menu201_draw);
    let puyo1_menu01_draw = verify_puyo1_menu01_draw_route(
        &btx_name_consumers.links,
        pending_btx_selections,
        &menu_static_cells,
        common_font_draw_sites,
    )
    .map_err(|error| error.to_string())?;
    print_classic_ui_draw_route("PUYO1 MENU01", &puyo1_menu01_draw);
    let puyo2_menu202_draw = verify_puyo2_menu202_draw_route(
        &btx_name_consumers.links,
        pending_btx_selections,
        &menu_static_cells,
        common_font_draw_sites,
    )
    .map_err(|error| error.to_string())?;
    print_classic_ui_draw_route("PUYO2 MENU202", &puyo2_menu202_draw);
    let puyo2_menu203_draw = verify_puyo2_menu203_draw_route(
        &btx_name_consumers.links,
        pending_btx_selections,
        &menu_static_cells,
        common_font_draw_sites,
    )
    .map_err(|error| error.to_string())?;
    print_classic_ui_draw_route("PUYO2 MENU203", &puyo2_menu203_draw);
    let puyo2_p2ruled1_draw = verify_puyo2_p2ruled1_draw_route(
        &btx_name_consumers.links,
        pending_btx_selections,
        &p2ruled1_static_cells,
        common_font_draw_sites,
    )
    .map_err(|error| error.to_string())?;
    print_classic_ui_draw_route("PUYO2 P2RULED1", &puyo2_p2ruled1_draw);

    Ok(())
}
