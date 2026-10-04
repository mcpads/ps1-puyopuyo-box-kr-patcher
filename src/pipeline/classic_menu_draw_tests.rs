use super::*;
use crate::disasm::{BtxDescriptorSelection, CommonFontDrawSite};
use crate::pipeline::{BtxPendingStaticCellRow, BtxPendingStaticSetup};

fn link(spec: &ClassicMenuDrawSpec) -> BtxNameLink {
    BtxNameLink {
        exe: spec.exe,
        archive: spec.archive,
        asset_name: spec.asset_name.to_string(),
        exe_text_offset: 0,
        runtime_addr: 0,
        static_ref_sites: Vec::new(),
        entry_reached_ref_sites: Vec::new(),
        table_runtime_start: 0,
        table_runtime_end: 0,
        table_static_ref_sites: Vec::new(),
        table_entry_reached_ref_sites: Vec::new(),
        descriptor_pointer_offset: 0,
        descriptor_pointer_runtime: spec.descriptor_runtime + 4,
        descriptor_runtime_start: spec.descriptor_root_runtime,
        descriptor_runtime_end: spec.descriptor_runtime + DESCRIPTOR_BYTES,
        descriptor_static_ref_sites: Vec::new(),
        descriptor_entry_reached_ref_sites: Vec::new(),
    }
}

fn selections(spec: &ClassicMenuDrawSpec) -> BtxDescriptorSelectionAnalysis {
    BtxDescriptorSelectionAnalysis {
        selections: vec![BtxDescriptorSelection {
            exe: spec.exe,
            archive: spec.archive,
            resource_multiply_runtime: spec.resource_multiply_runtime,
            descriptor_select_runtime: spec.descriptor_select_runtime,
            cache_check_runtime: spec.descriptor_cache_check_runtime,
            descriptor_copy_runtime: spec.descriptor_copy_runtime,
            descriptor_name_field_read_runtime: spec.descriptor_name_read_runtime,
            descriptor_copy_end_runtime: spec.descriptor_copy_runtime + 0x34,
            descriptor_copy_source_register: Reg::V1,
            working_descriptor_runtime: spec.working_descriptor_runtime,
            entry_reached: false,
        }],
    }
}

fn menu_cells(spec: &ClassicMenuDrawSpec) -> BtxPendingStaticCellAnalysis {
    let setup = BtxPendingStaticSetup {
        sequence: spec.sequence.to_string(),
        record_offset: spec.setup_record_offset,
        record_runtime: spec.setup_record_runtime,
        table_offset: spec.cell_table_offset,
        table_runtime: spec.cell_table_runtime,
        record_cell_count: 0,
        atlas_width: spec.atlas_width,
        atlas_height: spec.atlas_height,
        review_layout_matches: true,
        descriptor_plain_layout_matches: true,
        next_referenced_table_offset: Some(spec.next_cell_table_offset),
        next_referenced_overlay_offset: None,
        next_overlay_cell_span: None,
        inferred_cell_records: Some(spec.inferred_cell_records),
        table_padding_bytes: Some(0),
        valid_cells: Some(spec.valid_cells),
        empty_cells: Some(spec.empty_cells),
        outside_cells: Some(0),
        inferred_cells: vec![spec.cell_record],
        leading_cell_prefix: None,
    };
    BtxPendingStaticCellAnalysis {
        rows: vec![BtxPendingStaticCellRow {
            finding_id: "fixture".to_string(),
            exe: spec.exe,
            grp_archive: spec.archive,
            seq_archive: "fixture",
            asset_name: spec.asset_name.to_string(),
            resource_id: spec.resource_id,
            descriptor_runtime: spec.descriptor_runtime,
            descriptor_stored_bytes: 0,
            descriptor_plain_bytes: 0,
            initial_cell_table_runtime: 0,
            initial_cell_count: 0,
            setups: vec![setup],
        }],
        findings: 1,
        physical_assets: 1,
        setup_records: 1,
        rows_with_setup: 1,
        layout_matching_setup_records: 1,
        plain_layout_matching_setup_records: 1,
    }
}

fn draw_sites(spec: &ClassicMenuDrawSpec) -> CommonFontDrawSiteAnalysis {
    CommonFontDrawSiteAnalysis {
        sites: vec![CommonFontDrawSite {
            exe: spec.exe,
            archive: spec.archive,
            function_runtime: spec.short_renderer_runtime,
            renderer_source_bytes: 248,
            renderer_source_sha256: [0; 32],
            lookup_runtime: spec.cell_lookup_runtime,
            primitive_runtime: spec.primitive_runtime,
            primitive_target_runtime: spec.primitive_target_runtime,
            object_register: Reg::S4,
            descriptor_register: Reg::S3,
            cell_index_register: Reg::A0,
            cell_record_register: Reg::S5,
            cell_fields_register: Reg::S1,
            resource_field_offset: 0x006C,
            cell_field_offset: 0x006A,
        }],
        analysis_sha256: [0; 32],
    }
}

fn verify(spec: &ClassicMenuDrawSpec) -> Result<ClassicMenuDrawRoute, ClassicMenuDrawError> {
    verify_classic_menu_draw_route(
        spec,
        &[link(spec)],
        &selections(spec),
        &menu_cells(spec),
        &draw_sites(spec),
    )
}

#[test]
fn puyo1_selector_34는_header_다음의_모드_block을_그린다() {
    let route = verify(&PUYO1_MENU01).unwrap();

    assert_eq!(route.resource_id, 110);
    assert_eq!(route.cell_selector, 34);
    assert_eq!(route.cell_record_index, 35);
    assert_eq!(route.cell_record_runtime, 0x801A_94EC);
    assert_eq!(route.text_block_id, "BTXB-0177");
    assert_eq!(route.source_text, "モード");
    assert_eq!(route.primitive_runtime, 0x8002_4BA4);
}

#[test]
fn puyo2_selector_22는_header_다음의_선택_히토리데_block을_그린다() {
    let route = verify(&PUYO2_MENU202).unwrap();

    assert_eq!(route.resource_id, 84);
    assert_eq!(route.descriptor_runtime, 0x8006_4A6C);
    assert_eq!(route.setup_record_runtime, 0x801A_718C);
    assert_eq!(route.cell_table_runtime, 0x801A_B6A0);
    assert_eq!(route.cell_selector, 22);
    assert_eq!(route.cell_record_index, 23);
    assert_eq!(route.cell_record_runtime, 0x801A_B7B4);
    assert_eq!(route.text_block_id, "BTXB-0204");
    assert_eq!(route.source_text, "ひとりで\nぷよぷよ");
    assert_eq!(route.primitive_runtime, 0x8002_3BE0);
}

#[test]
fn puyo2_menu201_selector_148은_header_다음의_히미츠_block을_그린다() {
    let route = verify(&PUYO2_MENU201).unwrap();

    assert_eq!(route.resource_id, 83);
    assert_eq!(route.descriptor_runtime, 0x8006_4A48);
    assert_eq!(route.setup_record_runtime, 0x801A_636C);
    assert_eq!(route.cell_table_runtime, 0x801A_AEC0);
    assert_eq!(route.cell_selector, 148);
    assert_eq!(route.cell_record_index, 149);
    assert_eq!(route.cell_record_runtime, 0x801A_B5BC);
    assert_eq!(route.text_block_id, "BTXB-0214");
    assert_eq!(route.source_text, "のひみつ");
    assert_eq!(route.primitive_runtime, 0x8002_3BE0);
}

#[test]
fn puyo2_menu203_selector_18은_header_다음의_rule_block을_그린다() {
    let route = verify(&PUYO2_MENU203).unwrap();

    assert_eq!(route.resource_id, 85);
    assert_eq!(route.descriptor_runtime, 0x8006_4A90);
    assert_eq!(route.setup_record_runtime, 0x801A_8088);
    assert_eq!(route.cell_table_runtime, 0x801A_BA3C);
    assert_eq!(route.cell_selector, 18);
    assert_eq!(route.cell_record_index, 19);
    assert_eq!(route.cell_record_runtime, 0x801A_BB20);
    assert_eq!(route.text_block_id, "BTXB-0192");
    assert_eq!(route.source_text, "ルール");
    assert_eq!(route.primitive_runtime, 0x8002_3BE0);
}

#[test]
fn puyo2_p2ruled1_selector_33은_규칙편집_질문_block을_그린다() {
    let mut cells = menu_cells(&PUYO2_P2RULED1);
    let mut alternate_setup = cells.rows[0].setups[0].clone();
    alternate_setup.record_offset += 4;
    cells.rows[0].setups.push(alternate_setup);

    let route = verify_classic_menu_draw_route(
        &PUYO2_P2RULED1,
        &[link(&PUYO2_P2RULED1)],
        &selections(&PUYO2_P2RULED1),
        &cells,
        &draw_sites(&PUYO2_P2RULED1),
    )
    .unwrap();

    assert_eq!(route.resource_id, 91);
    assert_eq!(route.descriptor_runtime, 0x8006_4B68);
    assert_eq!(route.setup_record_runtime, 0x801A_A428);
    assert_eq!(route.cell_table_runtime, 0x801A_C4A4);
    assert_eq!(route.cell_selector, 33);
    assert_eq!(route.cell_record_index, 34);
    assert_eq!(route.cell_record_runtime, 0x801A_C63C);
    assert_eq!(route.text_block_id, "BTXB-0158");
    assert_eq!(route.source_text, "なにをするの？");
    assert_eq!(route.primitive_runtime, 0x8002_3BE0);
}

#[test]
fn 다른_descriptor_resource를_actual_draw로_승인하지_않는다() {
    let mut wrong_link = link(&PUYO2_MENU202);
    wrong_link.descriptor_pointer_runtime += DESCRIPTOR_BYTES;

    let error = verify_classic_menu_draw_route(
        &PUYO2_MENU202,
        &[wrong_link],
        &selections(&PUYO2_MENU202),
        &menu_cells(&PUYO2_MENU202),
        &draw_sites(&PUYO2_MENU202),
    )
    .unwrap_err();
    assert!(error.to_string().contains("descriptor/resource"));
}

#[test]
fn 관측_cell_bbox가_바뀌면_승인하지_않는다() {
    let mut cells = menu_cells(&PUYO2_MENU202);
    cells.rows[0].setups[0].inferred_cells[0].width -= 1;

    let error = verify_classic_menu_draw_route(
        &PUYO2_MENU202,
        &[link(&PUYO2_MENU202)],
        &selections(&PUYO2_MENU202),
        &cells,
        &draw_sites(&PUYO2_MENU202),
    )
    .unwrap_err();
    assert!(error.to_string().contains("관측 cell"));
}

#[test]
fn short_renderer_primitive가_바뀌면_승인하지_않는다() {
    let mut sites = draw_sites(&PUYO2_MENU202);
    sites.sites[0].primitive_runtime += 4;

    let error = verify_classic_menu_draw_route(
        &PUYO2_MENU202,
        &[link(&PUYO2_MENU202)],
        &selections(&PUYO2_MENU202),
        &menu_cells(&PUYO2_MENU202),
        &sites,
    )
    .unwrap_err();
    assert!(error.to_string().contains("short renderer"));
}
