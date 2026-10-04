use serde_json::json;

use super::*;
use crate::coords::LogicalFile;
use crate::disasm::CommonFontCellSourceRow;
use crate::font::RuntimeCell;
use crate::isa::{encode, InstructionKind as K, Reg};

const WRITER_RUNTIME: u32 = 0x8001_0020;
const OBJECT_RUNTIME: u32 = 0x800D_0000;
const CELL: u16 = 202;
const RESOURCE: u16 = 40;

fn map() -> ExeMap {
    ExeMap {
        name: "QUEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x100,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, 0x900),
    }
}

fn site() -> CommonFontDrawSite {
    CommonFontDrawSite {
        exe: "QUEST.EXE",
        archive: "/QUEST/RPG_GRP.VDD;1",
        function_runtime: 0x8002_0000,
        renderer_source_bytes: 4,
        renderer_source_sha256: [0; 32],
        lookup_runtime: 0x8002_0010,
        primitive_runtime: 0x8002_0020,
        primitive_target_runtime: 0x8003_0000,
        object_register: Reg::S5,
        descriptor_register: Reg::S4,
        cell_index_register: Reg::A0,
        cell_record_register: Reg::S6,
        cell_fields_register: Reg::S3,
        resource_field_offset: 0x0068,
        cell_field_offset: 0x0066,
    }
}

fn source_analysis() -> CommonFontCellSourceAnalysis {
    CommonFontCellSourceAnalysis {
        rows: vec![CommonFontCellSourceRow {
            surface_id: "quest-break-label",
            exe: "QUEST.EXE",
            archive: "/QUEST/RPG_GRP.VDD;1",
            source_text: "きゅうけい",
            candidate_cells: vec![202, 203],
            common_font_resource_id: RESOURCE,
            matched_cell: Some(CELL),
            cell_store_runtime: Some(WRITER_RUNTIME),
            resource_store_runtimes: vec![WRITER_RUNTIME - 4],
            entry_reached: false,
            static_verdict: "bounded-literal-cell-and-com-font-resource-writer",
            runtime_draw_verified: false,
        }],
        resolved_surfaces: 1,
        unresolved_surfaces: 0,
        literal_writer_sites: 1,
        entry_reached_writer_sites: 0,
        analysis_sha256: [0; 32],
    }
}

fn exe_text(cell_offset: i16) -> Vec<u8> {
    let mut text = vec![0; 0x100];
    text[0x20..0x24].copy_from_slice(
        &encode(K::Sh {
            rt: Reg::S1,
            base: Reg::V1,
            offset: cell_offset,
        })
        .to_le_bytes(),
    );
    text
}

fn write_u16(ram: &mut [u8], runtime: u32, value: u16) {
    let offset = psx_main_ram_offset(runtime, 2).unwrap();
    ram[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn ram(resource: u16) -> Vec<u8> {
    let mut ram = vec![0; PSX_MAIN_RAM_LEN];
    write_u16(&mut ram, OBJECT_RUNTIME + 0x66, CELL);
    write_u16(&mut ram, OBJECT_RUNTIME + 0x68, resource);
    ram
}

fn draw_report(ram: &[u8], object_runtime: u32) -> CommonFontDrawReport {
    CommonFontDrawReport {
        schema_version: "common-font-draw-runtime-v2",
        exe: "QUEST.EXE",
        archive: "/QUEST/RPG_GRP.VDD;1",
        renderer_source_sha256: "renderer".to_string(),
        ram_sha256: digest(ram),
        event_sha256: "draw-events".to_string(),
        screen_sha256: "screen".to_string(),
        descriptor_runtime: 0x8008_CE98,
        table_runtime: 0x8008_3A74,
        atlas_width: 256,
        atlas_height: 128,
        record_count: 251,
        event_count: 2,
        observed_event_count: 2,
        lookup_hits: 1,
        primitive_hits: 1,
        clipped_hits: 0,
        renderer_return_runtimes: vec![0x8002_37E0],
        japanese_surface_candidate_cells_observed: 1,
        screen_capture_shape_exact: true,
        final_pixel_attribution_verified: false,
        runtime_verified: true,
        release_eligible: false,
        rows: vec![super::super::CommonFontDrawRow {
            cell: RuntimeCell {
                index: usize::from(CELL),
                x: 0,
                y: 64,
                width: 64,
                height: 24,
                anchor_x: 0,
                anchor_y: 0,
            },
            source_glyph: None,
            semantic_category: "japanese-baked-surface-candidate",
            translation_disposition: "translate-japanese-candidate",
            japanese_surface_ids: vec!["quest-break-label"],
            japanese_source_texts: vec!["きゅうけい"],
            lookup_hits: 1,
            primitive_hits: 1,
            clipped_hits: 0,
            object_count: 1,
            primitive_object_runtimes: vec![object_runtime],
        }],
    }
}

fn writer_events(dropped: u64) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "dropped": dropped,
        "events": [{
            "pc": WRITER_RUNTIME,
            "registers": {"s1": CELL, "v1": OBJECT_RUNTIME}
        }]
    }))
    .unwrap()
}

fn inspect(
    text: &[u8],
    ram: &[u8],
    draw: &CommonFontDrawReport,
    events: &[u8],
) -> Result<CommonFontWriterDrawReport, CommonFontWriterDrawError> {
    inspect_common_font_writer_draw(CommonFontWriterDrawObservationInput {
        map: &map(),
        exe_text: text,
        site: &site(),
        cell_sources: &source_analysis(),
        draw_report: draw,
        ram,
        writer_events_json: events,
    })
}

#[test]
fn 같은_object의_writer와_primitive_draw를_결속한다() {
    let ram = ram(RESOURCE);
    let draw = draw_report(&ram, OBJECT_RUNTIME);
    let report = inspect(&exe_text(0x66), &ram, &draw, &writer_events(0))
        .expect("같은 object의 writer와 primitive draw는 결속해야 한다");

    assert_eq!(report.candidate_writer_sites, 1);
    assert_eq!(report.observed_writer_sites, 1);
    assert_eq!(report.writer_object_count, 1);
    assert_eq!(report.draw_object_count, 1);
    assert!(report.terminal_object_fields_verified);
    assert!(report.writer_draw_object_sets_equal);
    assert!(report.runtime_verified);
    assert!(!report.final_pixel_attribution_verified);
    assert!(!report.release_eligible);
    assert_eq!(report.rows[0].writer_runtime, WRITER_RUNTIME);
    assert_eq!(report.rows[0].object_runtime, OBJECT_RUNTIME);

    let tsv = render_common_font_writer_draw_tsv(&report);
    assert!(tsv.contains("quest-break-label\tきゅうけい\t202\t40"));
    assert!(tsv.contains("0x80010020\t0x8001001C\t0x800D0000"));
}

#[test]
fn writer와_draw의_object가_다르면_거부한다() {
    let ram = ram(RESOURCE);
    let draw = draw_report(&ram, OBJECT_RUNTIME + 0x100);
    let error = inspect(&exe_text(0x66), &ram, &draw, &writer_events(0)).unwrap_err();
    assert!(error.to_string().contains("primitive draw 집합에 없음"));
}

#[test]
fn object의_종단resource가_다르면_거부한다() {
    let ram = ram(RESOURCE + 1);
    let draw = draw_report(&ram, OBJECT_RUNTIME);
    let error = inspect(&exe_text(0x66), &ram, &draw, &writer_events(0)).unwrap_err();
    assert!(error.to_string().contains("종단 resource 불일치"));
}

#[test]
fn 원본writer명령과_dropped_event를_거부한다() {
    let ram = ram(RESOURCE);
    let draw = draw_report(&ram, OBJECT_RUNTIME);
    let instruction_error = inspect(&exe_text(0x64), &ram, &draw, &writer_events(0)).unwrap_err();
    assert!(instruction_error.to_string().contains("cell field 불일치"));

    let dropped_error = inspect(&exe_text(0x66), &ram, &draw, &writer_events(1)).unwrap_err();
    assert!(dropped_error.to_string().contains("dropped=1"));
}
