//! PUYO1/PUYO2 classic UI의 baked label draw 결속.
//!
//! 지원 원본의 이름표·descriptor 선택, SEQ cell table, EXE별
//! short renderer를 교차한다. runtime 화면·register·VRAM 증거는 이 source
//! 검증과 별도로 보존하고, 여기서는 관측값이 연결되는 불변 원본 경로만
//! 검산한다.

use std::error::Error;
use std::fmt;

use crate::disasm::{BtxDescriptorSelectionAnalysis, BtxNameLink, CommonFontDrawSiteAnalysis};
use crate::graphics::text_catalog::{
    TextAssetSpec, MENU01_TEXT_ASSET, MENU201_TEXT_ASSET, MENU202_TEXT_ASSET, MENU203_TEXT_ASSET,
    P2RULED1_TEXT_ASSET,
};
use crate::isa::Reg;

use super::{BtxPendingStaticCellAnalysis, BtxPendingStaticCellRecord};

const DESCRIPTOR_BYTES: u32 = 36;
const CELL_RECORD_BYTES: u32 = 12;
const CELL_HEADER_RECORDS: usize = 1;

#[derive(Debug, Clone, Copy)]
struct ClassicMenuDrawSpec {
    route_name: &'static str,
    exe: &'static str,
    archive: &'static str,
    asset_name: &'static str,
    resource_id: u16,
    descriptor_root_runtime: u32,
    descriptor_runtime: u32,
    resource_multiply_runtime: u32,
    descriptor_select_runtime: u32,
    descriptor_cache_check_runtime: u32,
    descriptor_copy_runtime: u32,
    descriptor_name_read_runtime: u32,
    descriptor_cell_table_read_runtime: u32,
    working_descriptor_runtime: u32,
    sequence: &'static str,
    setup_record_offset: usize,
    setup_record_runtime: u32,
    cell_table_offset: usize,
    cell_table_runtime: u32,
    next_cell_table_offset: usize,
    atlas_width: u16,
    atlas_height: u16,
    inferred_cell_records: usize,
    valid_cells: usize,
    empty_cells: usize,
    cell_selector: usize,
    cell_record: BtxPendingStaticCellRecord,
    text_block_id: &'static str,
    source_text: &'static str,
    text_asset: &'static TextAssetSpec<'static>,
    short_renderer_runtime: u32,
    cell_lookup_runtime: u32,
    primitive_runtime: u32,
    primitive_target_runtime: u32,
}

const PUYO1_MENU01: ClassicMenuDrawSpec = ClassicMenuDrawSpec {
    route_name: "PUYO1 MENU01",
    exe: "PUYO1.EXE",
    archive: "/PUYO1/PP1_GRP.VDD;1",
    asset_name: "MENU01.BTX",
    resource_id: 110,
    descriptor_root_runtime: 0x8006_7E48,
    descriptor_runtime: 0x8006_8DC0,
    resource_multiply_runtime: 0x8001_AEFC,
    descriptor_select_runtime: 0x8001_AF0C,
    descriptor_cache_check_runtime: 0x8001_AF1C,
    descriptor_copy_runtime: 0x8001_AF30,
    descriptor_name_read_runtime: 0x8001_AF34,
    descriptor_cell_table_read_runtime: 0x8001_AF3C,
    working_descriptor_runtime: 0x800B_37C0,
    sequence: "MENU1.SEQ",
    setup_record_offset: 0x01B4,
    setup_record_runtime: 0x801A_6424,
    cell_table_offset: 0x30D8,
    cell_table_runtime: 0x801A_9348,
    next_cell_table_offset: 0x32DC,
    atlas_width: 512,
    atlas_height: 256,
    inferred_cell_records: 42,
    valid_cells: 42,
    empty_cells: 0,
    cell_selector: 34,
    cell_record: BtxPendingStaticCellRecord {
        index: 34,
        x: 0,
        y: 16,
        width: 48,
        height: 16,
        anchor_x: 0,
        anchor_y: 0,
        empty: false,
        outside_atlas: false,
    },
    text_block_id: "BTXB-0177",
    source_text: "モード",
    text_asset: &MENU01_TEXT_ASSET,
    short_renderer_runtime: 0x8002_4AB4,
    cell_lookup_runtime: 0x8002_4B10,
    primitive_runtime: 0x8002_4BA4,
    primitive_target_runtime: 0x8007_266C,
};

const PUYO2_MENU202: ClassicMenuDrawSpec = ClassicMenuDrawSpec {
    route_name: "PUYO2 MENU202",
    exe: "PUYO2.EXE",
    archive: "/PUYO2/PP2_GRP.VDD;1",
    asset_name: "MENU202.BTX",
    resource_id: 84,
    descriptor_root_runtime: 0x8006_3E9C,
    descriptor_runtime: 0x8006_4A6C,
    resource_multiply_runtime: 0x8001_9988,
    descriptor_select_runtime: 0x8001_9998,
    descriptor_cache_check_runtime: 0x8001_99A8,
    descriptor_copy_runtime: 0x8001_99BC,
    descriptor_name_read_runtime: 0x8001_99C0,
    descriptor_cell_table_read_runtime: 0x8001_99C8,
    working_descriptor_runtime: 0x800B_5BA0,
    sequence: "MENU2.SEQ",
    setup_record_offset: 0x0F1C,
    setup_record_runtime: 0x801A_718C,
    cell_table_offset: 0x5430,
    cell_table_runtime: 0x801A_B6A0,
    next_cell_table_offset: 0x57CC,
    atlas_width: 768,
    atlas_height: 256,
    inferred_cell_records: 76,
    valid_cells: 75,
    empty_cells: 1,
    cell_selector: 22,
    cell_record: BtxPendingStaticCellRecord {
        index: 22,
        x: 104,
        y: 0,
        width: 80,
        height: 48,
        anchor_x: 0,
        anchor_y: 0,
        empty: false,
        outside_atlas: false,
    },
    text_block_id: "BTXB-0204",
    source_text: "ひとりで\nぷよぷよ",
    text_asset: &MENU202_TEXT_ASSET,
    short_renderer_runtime: 0x8002_3AF0,
    cell_lookup_runtime: 0x8002_3B4C,
    primitive_runtime: 0x8002_3BE0,
    primitive_target_runtime: 0x8007_0BE8,
};

const PUYO2_MENU201: ClassicMenuDrawSpec = ClassicMenuDrawSpec {
    route_name: "PUYO2 MENU201",
    exe: "PUYO2.EXE",
    archive: "/PUYO2/PP2_GRP.VDD;1",
    asset_name: "MENU201.BTX",
    resource_id: 83,
    descriptor_root_runtime: 0x8006_3E9C,
    descriptor_runtime: 0x8006_4A48,
    resource_multiply_runtime: 0x8001_9988,
    descriptor_select_runtime: 0x8001_9998,
    descriptor_cache_check_runtime: 0x8001_99A8,
    descriptor_copy_runtime: 0x8001_99BC,
    descriptor_name_read_runtime: 0x8001_99C0,
    descriptor_cell_table_read_runtime: 0x8001_99C8,
    working_descriptor_runtime: 0x800B_5BA0,
    sequence: "MENU2.SEQ",
    setup_record_offset: 0x00FC,
    setup_record_runtime: 0x801A_636C,
    cell_table_offset: 0x4C50,
    cell_table_runtime: 0x801A_AEC0,
    next_cell_table_offset: 0x5430,
    atlas_width: 512,
    atlas_height: 256,
    inferred_cell_records: 167,
    valid_cells: 151,
    empty_cells: 16,
    cell_selector: 148,
    cell_record: BtxPendingStaticCellRecord {
        index: 148,
        x: 0,
        y: 136,
        width: 64,
        height: 16,
        anchor_x: 0,
        anchor_y: 0,
        empty: false,
        outside_atlas: false,
    },
    text_block_id: "BTXB-0214",
    source_text: "のひみつ",
    text_asset: &MENU201_TEXT_ASSET,
    short_renderer_runtime: 0x8002_3AF0,
    cell_lookup_runtime: 0x8002_3B4C,
    primitive_runtime: 0x8002_3BE0,
    primitive_target_runtime: 0x8007_0BE8,
};

const PUYO2_MENU203: ClassicMenuDrawSpec = ClassicMenuDrawSpec {
    route_name: "PUYO2 MENU203",
    exe: "PUYO2.EXE",
    archive: "/PUYO2/PP2_GRP.VDD;1",
    asset_name: "MENU203.BTX",
    resource_id: 85,
    descriptor_root_runtime: 0x8006_3E9C,
    descriptor_runtime: 0x8006_4A90,
    resource_multiply_runtime: 0x8001_9988,
    descriptor_select_runtime: 0x8001_9998,
    descriptor_cache_check_runtime: 0x8001_99A8,
    descriptor_copy_runtime: 0x8001_99BC,
    descriptor_name_read_runtime: 0x8001_99C0,
    descriptor_cell_table_read_runtime: 0x8001_99C8,
    working_descriptor_runtime: 0x800B_5BA0,
    sequence: "MENU2.SEQ",
    setup_record_offset: 0x1E18,
    setup_record_runtime: 0x801A_8088,
    cell_table_offset: 0x57CC,
    cell_table_runtime: 0x801A_BA3C,
    next_cell_table_offset: 0x5A90,
    atlas_width: 256,
    atlas_height: 256,
    inferred_cell_records: 58,
    valid_cells: 56,
    empty_cells: 2,
    cell_selector: 18,
    cell_record: BtxPendingStaticCellRecord {
        index: 18,
        x: 152,
        y: 88,
        width: 72,
        height: 24,
        anchor_x: 0,
        anchor_y: 0,
        empty: false,
        outside_atlas: false,
    },
    text_block_id: "BTXB-0192",
    source_text: "ルール",
    text_asset: &MENU203_TEXT_ASSET,
    short_renderer_runtime: 0x8002_3AF0,
    cell_lookup_runtime: 0x8002_3B4C,
    primitive_runtime: 0x8002_3BE0,
    primitive_target_runtime: 0x8007_0BE8,
};

const PUYO2_P2RULED1: ClassicMenuDrawSpec = ClassicMenuDrawSpec {
    route_name: "PUYO2 P2RULED1",
    exe: "PUYO2.EXE",
    archive: "/PUYO2/PP2_GRP.VDD;1",
    asset_name: "P2RULED1.BTX",
    resource_id: 91,
    descriptor_root_runtime: 0x8006_3E9C,
    descriptor_runtime: 0x8006_4B68,
    resource_multiply_runtime: 0x8001_9988,
    descriptor_select_runtime: 0x8001_9998,
    descriptor_cache_check_runtime: 0x8001_99A8,
    descriptor_copy_runtime: 0x8001_99BC,
    descriptor_name_read_runtime: 0x8001_99C0,
    descriptor_cell_table_read_runtime: 0x8001_99C8,
    working_descriptor_runtime: 0x800B_5BA0,
    sequence: "MENU2.SEQ",
    setup_record_offset: 0x41B8,
    setup_record_runtime: 0x801A_A428,
    cell_table_offset: 0x6234,
    cell_table_runtime: 0x801A_C4A4,
    next_cell_table_offset: 0x6474,
    atlas_width: 256,
    atlas_height: 176,
    inferred_cell_records: 47,
    valid_cells: 45,
    empty_cells: 2,
    cell_selector: 33,
    cell_record: BtxPendingStaticCellRecord {
        index: 33,
        x: 0,
        y: 64,
        width: 112,
        height: 16,
        anchor_x: 0,
        anchor_y: 0,
        empty: false,
        outside_atlas: false,
    },
    text_block_id: "BTXB-0158",
    source_text: "なにをするの？",
    text_asset: &P2RULED1_TEXT_ASSET,
    short_renderer_runtime: 0x8002_3AF0,
    cell_lookup_runtime: 0x8002_3B4C,
    primitive_runtime: 0x8002_3BE0,
    primitive_target_runtime: 0x8007_0BE8,
};

/// 지원 원본에서 검산한 classic UI의 baked label draw 경로.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicMenuDrawRoute {
    pub exe: &'static str,
    pub archive: &'static str,
    pub asset_name: &'static str,
    pub resource_id: u16,
    pub descriptor_runtime: u32,
    pub descriptor_cell_table_read_runtime: u32,
    pub working_descriptor_runtime: u32,
    pub static_entry_reached: bool,
    pub setup_record_runtime: u32,
    pub cell_table_runtime: u32,
    pub cell_selector: usize,
    pub cell_record_index: usize,
    pub cell_record_runtime: u32,
    pub cell_record: BtxPendingStaticCellRecord,
    pub text_block_id: &'static str,
    pub source_text: &'static str,
    pub short_renderer_runtime: u32,
    pub cell_lookup_runtime: u32,
    pub primitive_runtime: u32,
    pub object_resource_field_offset: i16,
    pub object_cell_field_offset: i16,
}

/// classic menu draw 결속 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicMenuDrawError(String);

impl fmt::Display for ClassicMenuDrawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ClassicMenuDrawError {}

fn error(message: impl Into<String>) -> ClassicMenuDrawError {
    ClassicMenuDrawError(message.into())
}

fn verify_classic_menu_draw_route(
    spec: &ClassicMenuDrawSpec,
    links: &[BtxNameLink],
    selections: &BtxDescriptorSelectionAnalysis,
    menu_cells: &BtxPendingStaticCellAnalysis,
    draw_sites: &CommonFontDrawSiteAnalysis,
) -> Result<ClassicMenuDrawRoute, ClassicMenuDrawError> {
    let matching_links = links
        .iter()
        .filter(|link| {
            link.exe == spec.exe
                && link.archive == spec.archive
                && link.asset_name == spec.asset_name
        })
        .collect::<Vec<_>>();
    let [link] = matching_links.as_slice() else {
        return Err(error(format!(
            "{} 이름표 연결 수가 1이 아님: {}",
            spec.route_name,
            matching_links.len()
        )));
    };
    let descriptor_runtime = link
        .descriptor_pointer_runtime
        .checked_sub(4)
        .ok_or_else(|| error(format!("{} descriptor +4 주소 underflow", spec.route_name)))?;
    let descriptor_relative = descriptor_runtime
        .checked_sub(link.descriptor_runtime_start)
        .ok_or_else(|| {
            error(format!(
                "{} descriptor가 block 선두보다 앞임",
                spec.route_name
            ))
        })?;
    if !descriptor_relative.is_multiple_of(DESCRIPTOR_BYTES) {
        return Err(error(format!(
            "{} descriptor가 36바이트 resource 격자에 없음: +0x{descriptor_relative:X}",
            spec.route_name
        )));
    }
    let resource_id = u16::try_from(descriptor_relative / DESCRIPTOR_BYTES)
        .map_err(|_| error(format!("{} resource ID 범위 초과", spec.route_name)))?;
    if link.descriptor_runtime_start != spec.descriptor_root_runtime
        || link.descriptor_pointer_runtime != spec.descriptor_runtime + 4
        || descriptor_runtime != spec.descriptor_runtime
        || resource_id != spec.resource_id
    {
        return Err(error(format!(
            "{} descriptor/resource 불일치: root=0x{:08X}, pointer=0x{:08X}, descriptor=0x{descriptor_runtime:08X}, resource={resource_id}",
            spec.route_name, link.descriptor_runtime_start, link.descriptor_pointer_runtime
        )));
    }

    let matching_selections = selections
        .selections
        .iter()
        .filter(|selection| selection.exe == spec.exe && selection.archive == spec.archive)
        .collect::<Vec<_>>();
    let [selection] = matching_selections.as_slice() else {
        return Err(error(format!(
            "{} descriptor 선택 경계 수가 1이 아님: {}",
            spec.route_name,
            matching_selections.len()
        )));
    };
    if selection.resource_multiply_runtime != spec.resource_multiply_runtime
        || selection.descriptor_select_runtime != spec.descriptor_select_runtime
        || selection.cache_check_runtime != spec.descriptor_cache_check_runtime
        || selection.descriptor_copy_runtime != spec.descriptor_copy_runtime
        || selection.descriptor_name_field_read_runtime != spec.descriptor_name_read_runtime
        || selection.descriptor_copy_source_register != Reg::V1
        || selection.working_descriptor_runtime != spec.working_descriptor_runtime
        || selection.descriptor_copy_end_runtime < spec.descriptor_cell_table_read_runtime
    {
        return Err(error(format!(
            "{} descriptor 선택 경계 불일치: {selection:?}",
            spec.route_name
        )));
    }

    let matching_rows = menu_cells
        .rows
        .iter()
        .filter(|row| {
            row.exe == spec.exe
                && row.grp_archive == spec.archive
                && row.asset_name == spec.asset_name
        })
        .collect::<Vec<_>>();
    let [row] = matching_rows.as_slice() else {
        return Err(error(format!(
            "{} static cell 행 수가 1이 아님: {}",
            spec.route_name,
            matching_rows.len()
        )));
    };
    if row.resource_id != spec.resource_id || row.descriptor_runtime != spec.descriptor_runtime {
        return Err(error(format!(
            "{} static cell descriptor/resource 불일치: descriptor=0x{:08X}, resource={}",
            spec.route_name, row.descriptor_runtime, row.resource_id
        )));
    }
    let matching_setups = row
        .setups
        .iter()
        .filter(|setup| {
            setup.sequence == spec.sequence && setup.record_offset == spec.setup_record_offset
        })
        .collect::<Vec<_>>();
    let [setup] = matching_setups.as_slice() else {
        return Err(error(format!(
            "{} 지정 setup 수가 1이 아님: {}",
            spec.route_name,
            matching_setups.len()
        )));
    };
    if setup.sequence != spec.sequence
        || setup.record_offset != spec.setup_record_offset
        || setup.record_runtime != spec.setup_record_runtime
        || setup.table_offset != spec.cell_table_offset
        || setup.table_runtime != spec.cell_table_runtime
        || setup.next_referenced_table_offset != Some(spec.next_cell_table_offset)
        || setup.atlas_width != spec.atlas_width
        || setup.atlas_height != spec.atlas_height
        || !setup.review_layout_matches
        || !setup.descriptor_plain_layout_matches
        || setup.inferred_cell_records != Some(spec.inferred_cell_records)
        || setup.valid_cells != Some(spec.valid_cells)
        || setup.empty_cells != Some(spec.empty_cells)
        || setup.outside_cells != Some(0)
    {
        return Err(error(format!(
            "{} setup/cell table 불일치: {setup:?}",
            spec.route_name
        )));
    }
    let matching_cells = setup
        .inferred_cells
        .iter()
        .filter(|cell| cell.index == spec.cell_selector)
        .collect::<Vec<_>>();
    let [cell] = matching_cells.as_slice() else {
        return Err(error(format!(
            "{} selector {} cell 수가 1이 아님: {}",
            spec.route_name,
            spec.cell_selector,
            matching_cells.len()
        )));
    };
    if **cell != spec.cell_record {
        return Err(error(format!(
            "{} 관측 cell 불일치: expected={:?}, actual={cell:?}",
            spec.route_name, spec.cell_record
        )));
    }

    let matching_blocks = spec
        .text_asset
        .blocks
        .iter()
        .filter(|block| block.id == spec.text_block_id)
        .collect::<Vec<_>>();
    let [block] = matching_blocks.as_slice() else {
        return Err(error(format!(
            "{} {} text block 수가 1이 아님: {}",
            spec.route_name,
            spec.text_block_id,
            matching_blocks.len()
        )));
    };
    if spec.text_asset.name != spec.asset_name
        || !spec.text_asset.archive_paths.contains(&spec.archive)
        || block.source != spec.source_text
        || block.bbox.x != usize::from(cell.x)
        || block.bbox.y != usize::from(cell.y)
        || block.bbox.width != usize::from(cell.width)
        || block.bbox.height != usize::from(cell.height)
    {
        return Err(error(format!(
            "{} 관측 cell과 text block bbox가 일치하지 않음: cell={cell:?}, block={block:?}",
            spec.route_name
        )));
    }

    let matching_draw_sites = draw_sites
        .sites
        .iter()
        .filter(|site| site.exe == spec.exe && site.archive == spec.archive)
        .collect::<Vec<_>>();
    let [draw_site] = matching_draw_sites.as_slice() else {
        return Err(error(format!(
            "{} short renderer draw site 수가 1이 아님: {}",
            spec.route_name,
            matching_draw_sites.len()
        )));
    };
    if draw_site.function_runtime != spec.short_renderer_runtime
        || draw_site.lookup_runtime != spec.cell_lookup_runtime
        || draw_site.primitive_runtime != spec.primitive_runtime
        || draw_site.primitive_target_runtime != spec.primitive_target_runtime
        || draw_site.object_register != Reg::S4
        || draw_site.descriptor_register != Reg::S3
        || draw_site.cell_index_register != Reg::A0
        || draw_site.cell_record_register != Reg::S5
        || draw_site.cell_fields_register != Reg::S1
        || draw_site.resource_field_offset != 0x006C
        || draw_site.cell_field_offset != 0x006A
    {
        return Err(error(format!(
            "{} short renderer 경계 불일치: {draw_site:?}",
            spec.route_name
        )));
    }

    let cell_record_index = spec.cell_selector + CELL_HEADER_RECORDS;
    let cell_record_runtime = spec.cell_table_runtime
        + u32::try_from(cell_record_index).expect("cell record index는 u32 범위")
            * CELL_RECORD_BYTES;
    Ok(ClassicMenuDrawRoute {
        exe: spec.exe,
        archive: spec.archive,
        asset_name: spec.asset_name,
        resource_id,
        descriptor_runtime,
        descriptor_cell_table_read_runtime: spec.descriptor_cell_table_read_runtime,
        working_descriptor_runtime: selection.working_descriptor_runtime,
        static_entry_reached: selection.entry_reached,
        setup_record_runtime: setup.record_runtime,
        cell_table_runtime: setup.table_runtime,
        cell_selector: spec.cell_selector,
        cell_record_index,
        cell_record_runtime,
        cell_record: **cell,
        text_block_id: block.id,
        source_text: block.source,
        short_renderer_runtime: draw_site.function_runtime,
        cell_lookup_runtime: draw_site.lookup_runtime,
        primitive_runtime: draw_site.primitive_runtime,
        object_resource_field_offset: draw_site.resource_field_offset,
        object_cell_field_offset: draw_site.cell_field_offset,
    })
}

/// PUYO1 `MENU01.BTX`의 `モード` actual-draw source 경로를 검증한다.
pub fn verify_puyo1_menu01_draw_route(
    links: &[BtxNameLink],
    selections: &BtxDescriptorSelectionAnalysis,
    menu_cells: &BtxPendingStaticCellAnalysis,
    draw_sites: &CommonFontDrawSiteAnalysis,
) -> Result<ClassicMenuDrawRoute, ClassicMenuDrawError> {
    verify_classic_menu_draw_route(&PUYO1_MENU01, links, selections, menu_cells, draw_sites)
}

/// PUYO2 `MENU202.BTX`의 선택 `ひとりで` actual-draw source 경로를 검증한다.
pub fn verify_puyo2_menu202_draw_route(
    links: &[BtxNameLink],
    selections: &BtxDescriptorSelectionAnalysis,
    menu_cells: &BtxPendingStaticCellAnalysis,
    draw_sites: &CommonFontDrawSiteAnalysis,
) -> Result<ClassicMenuDrawRoute, ClassicMenuDrawError> {
    verify_classic_menu_draw_route(&PUYO2_MENU202, links, selections, menu_cells, draw_sites)
}

/// PUYO2 `MENU201.BTX`의 도움말 `のひみつ` actual-draw source 경로를 검증한다.
pub fn verify_puyo2_menu201_draw_route(
    links: &[BtxNameLink],
    selections: &BtxDescriptorSelectionAnalysis,
    menu_cells: &BtxPendingStaticCellAnalysis,
    draw_sites: &CommonFontDrawSiteAnalysis,
) -> Result<ClassicMenuDrawRoute, ClassicMenuDrawError> {
    verify_classic_menu_draw_route(&PUYO2_MENU201, links, selections, menu_cells, draw_sites)
}

/// PUYO2 `MENU203.BTX`의 rule-select `ルール` actual-draw source 경로를 검증한다.
pub fn verify_puyo2_menu203_draw_route(
    links: &[BtxNameLink],
    selections: &BtxDescriptorSelectionAnalysis,
    menu_cells: &BtxPendingStaticCellAnalysis,
    draw_sites: &CommonFontDrawSiteAnalysis,
) -> Result<ClassicMenuDrawRoute, ClassicMenuDrawError> {
    verify_classic_menu_draw_route(&PUYO2_MENU203, links, selections, menu_cells, draw_sites)
}

/// PUYO2 `P2RULED1.BTX` 규칙 편집기의 `なにをするの？` actual-draw source 경로를 검증한다.
pub fn verify_puyo2_p2ruled1_draw_route(
    links: &[BtxNameLink],
    selections: &BtxDescriptorSelectionAnalysis,
    static_cells: &BtxPendingStaticCellAnalysis,
    draw_sites: &CommonFontDrawSiteAnalysis,
) -> Result<ClassicMenuDrawRoute, ClassicMenuDrawError> {
    verify_classic_menu_draw_route(&PUYO2_P2RULED1, links, selections, static_cells, draw_sites)
}

#[cfg(test)]
#[path = "classic_menu_draw_tests.rs"]
mod classic_menu_draw_tests;
