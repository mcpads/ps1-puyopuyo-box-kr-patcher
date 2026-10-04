use serde_json::{json, Map};

use super::*;

const DESCRIPTOR_RUNTIME: u32 = 0x8006_0000;
const TABLE_RUNTIME: u32 = 0x8010_0000;
const OBJECT_RUNTIME: u32 = 0x801F_F000;
const RETURN_RUNTIME: u32 = 0x8004_0000;

fn write_u16(ram: &mut [u8], runtime: u32, value: u16) {
    let offset = psx_main_ram_offset(runtime, 2).unwrap();
    ram[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(ram: &mut [u8], runtime: u32, value: u32) {
    let offset = psx_main_ram_offset(runtime, 4).unwrap();
    ram[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn site(exe: &'static str, renderer: &[u8]) -> CommonFontDrawSite {
    let renderer_source_sha256 = Sha256::digest(renderer).into();
    match exe {
        "PUYO2.EXE" => CommonFontDrawSite {
            exe,
            archive: "/PUYO2/PP2_GRP.VDD;1",
            function_runtime: 0x8002_3AF0,
            renderer_source_bytes: renderer.len(),
            renderer_source_sha256,
            lookup_runtime: 0x8002_3B4C,
            primitive_runtime: 0x8002_3BE0,
            primitive_target_runtime: 0x8007_0BE8,
            object_register: Reg::S4,
            descriptor_register: Reg::S3,
            cell_index_register: Reg::A0,
            cell_record_register: Reg::S5,
            cell_fields_register: Reg::S1,
            resource_field_offset: 0x006C,
            cell_field_offset: 0x006A,
        },
        "GOCHA1.EXE" => CommonFontDrawSite {
            exe,
            archive: "/GOCHA1/TUU_GRP.VDD;1",
            function_runtime: 0x8002_B504,
            renderer_source_bytes: renderer.len(),
            renderer_source_sha256,
            lookup_runtime: 0x8002_B564,
            primitive_runtime: 0x8002_B5F8,
            primitive_target_runtime: 0x8007_C010,
            object_register: Reg::S5,
            descriptor_register: Reg::S4,
            cell_index_register: Reg::A0,
            cell_record_register: Reg::S6,
            cell_fields_register: Reg::S3,
            resource_field_offset: 0x006C,
            cell_field_offset: 0x006A,
        },
        _ => unreachable!("테스트 site"),
    }
}

fn fixture(exe: &'static str) -> (CommonFontDrawSite, Vec<u8>, Vec<u8>, usize) {
    let renderer = (0..64)
        .map(|index| u8::try_from(index).unwrap())
        .collect::<Vec<_>>();
    let site = site(exe, &renderer);
    let (height, count) = match exe {
        "PUYO2.EXE" => (128, 251),
        "GOCHA1.EXE" => (80, 212),
        _ => unreachable!("테스트 fixture"),
    };
    let mut ram = vec![0; PSX_MAIN_RAM_LEN];
    let renderer_offset =
        psx_main_ram_offset(site.function_runtime, renderer.len()).expect("renderer RAM");
    ram[renderer_offset..renderer_offset + renderer.len()].copy_from_slice(&renderer);

    write_u32(&mut ram, DESCRIPTOR_RUNTIME, 0x0001_0001);
    write_u32(&mut ram, DESCRIPTOR_RUNTIME + 28, TABLE_RUNTIME);
    for (offset, value) in [(0, 0), (2, 0), (4, 256), (6, height), (8, 0), (10, 0)] {
        write_u16(&mut ram, TABLE_RUNTIME + offset, value);
    }
    for index in 0..count {
        let record = TABLE_RUNTIME + 12 + u32::try_from(index * 12).unwrap();
        write_u16(&mut ram, record, 0);
        write_u16(&mut ram, record + 2, 0);
        write_u16(&mut ram, record + 4, 8);
        write_u16(&mut ram, record + 6, 16);
    }
    for (index, x, y, width, cell_height) in [
        (0usize, 0, 0, 8, 16),
        (1, 8, 0, 8, 16),
        (26, 176, 0, 24, 8),
        (27, 216, 0, 24, 8),
        (32, 0, 16, 8, 16),
        (202, 208, 64, 48, 16),
        (203, 0, 64, 96, 32),
    ] {
        if index >= count || y + cell_height > height {
            continue;
        }
        let record = TABLE_RUNTIME + 12 + u32::try_from(index * 12).unwrap();
        write_u16(&mut ram, record, x);
        write_u16(&mut ram, record + 2, y);
        write_u16(&mut ram, record + 4, width);
        write_u16(&mut ram, record + 6, cell_height);
    }
    (site, ram, renderer, count)
}

fn event_registers(
    site: &CommonFontDrawSite,
    object: u32,
    return_runtime: u32,
) -> Map<String, Value> {
    let mut registers = Map::new();
    registers.insert(
        register_name(site.descriptor_register).unwrap().to_string(),
        json!(DESCRIPTOR_RUNTIME),
    );
    registers.insert(
        register_name(site.object_register).unwrap().to_string(),
        json!(object),
    );
    registers.insert("ra".to_string(), json!(return_runtime));
    registers
}

fn lookup(site: &CommonFontDrawSite, cell: u32, object: u32) -> Value {
    let mut registers = event_registers(site, object, RETURN_RUNTIME);
    registers.insert(
        register_name(site.cell_index_register).unwrap().to_string(),
        json!(cell),
    );
    json!({"pc": site.lookup_runtime, "registers": registers})
}

fn primitive_with_return(
    site: &CommonFontDrawSite,
    cell: u32,
    object: u32,
    return_runtime: u32,
) -> Value {
    let record = TABLE_RUNTIME + cell * 12;
    let mut registers = event_registers(site, object, return_runtime);
    registers.insert(
        register_name(site.cell_record_register)
            .unwrap()
            .to_string(),
        json!(record),
    );
    registers.insert(
        register_name(site.cell_fields_register)
            .unwrap()
            .to_string(),
        json!(record + 12),
    );
    json!({"pc": site.primitive_runtime, "registers": registers})
}

fn primitive(site: &CommonFontDrawSite, cell: u32, object: u32) -> Value {
    primitive_with_return(site, cell, object, RETURN_RUNTIME)
}

fn events(values: Vec<Value>, dropped: u64) -> Vec<u8> {
    serde_json::to_vec(&json!({"dropped": dropped, "events": values})).unwrap()
}

fn png() -> Vec<u8> {
    let mut raw = vec![0; 24];
    raw[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    raw[12..16].copy_from_slice(b"IHDR");
    raw[16..20].copy_from_slice(&SCREEN_WIDTH.to_be_bytes());
    raw[20..24].copy_from_slice(&SCREEN_HEIGHT.to_be_bytes());
    raw
}

fn inspect(
    site: &CommonFontDrawSite,
    ram: &[u8],
    renderer: &[u8],
    cell_count: usize,
    events_json: &[u8],
) -> Result<CommonFontDrawReport, CommonFontDrawError> {
    inspect_common_font_draw(CommonFontDrawObservationInput {
        site,
        descriptor_runtime: DESCRIPTOR_RUNTIME,
        cell_count,
        ram,
        renderer_source: renderer,
        events_json,
        screen_png: &png(),
    })
}

#[test]
fn puyo계열_register로_lookup과_primitive를_결속한다() {
    let (site, ram, renderer, count) = fixture("PUYO2.EXE");
    let raw_events = events(
        vec![
            lookup(&site, 0, OBJECT_RUNTIME),
            primitive(&site, 0, OBJECT_RUNTIME),
            lookup(&site, 32, OBJECT_RUNTIME + 64),
            primitive(&site, 32, OBJECT_RUNTIME + 64),
        ],
        0,
    );
    let report = inspect(&site, &ram, &renderer, count, &raw_events).unwrap();
    assert_eq!(report.lookup_hits, 2);
    assert_eq!(report.primitive_hits, 2);
    assert_eq!(report.clipped_hits, 0);
    assert_eq!(report.renderer_return_runtimes, vec![RETURN_RUNTIME]);
    assert_eq!(report.japanese_surface_candidate_cells_observed, 0);
    assert_eq!(report.rows[0].source_glyph, Some('0'));
    assert_eq!(report.rows[1].source_glyph, Some('A'));
    assert!(report.runtime_verified);
    assert!(!report.final_pixel_attribution_verified);
    assert!(!report.release_eligible);
    let tsv = render_common_font_draw_tsv(&report);
    assert!(tsv.contains("digit-glyph\tprotected-non-japanese-reference"));
    assert!(tsv.contains("latin-uppercase-glyph\tprotected-non-japanese-reference"));
}

#[test]
fn gocha계열_register와_일본어_surface후보를_해석한다() {
    let (site, ram, renderer, count) = fixture("GOCHA1.EXE");
    let raw_events = events(
        vec![
            lookup(&site, 26, OBJECT_RUNTIME),
            primitive(&site, 26, OBJECT_RUNTIME),
        ],
        0,
    );
    let report = inspect(&site, &ram, &renderer, count, &raw_events).unwrap();

    assert_eq!(report.japanese_surface_candidate_cells_observed, 1);
    assert_eq!(
        report.rows[0].japanese_surface_ids,
        vec!["gocha1-chain-label"]
    );
    assert_eq!(report.rows[0].japanese_source_texts, vec!["れんさ"]);
    assert_eq!(
        report.rows[0].translation_disposition,
        "translate-japanese-candidate"
    );
}

#[test]
fn clipping된_lookup을_primitive성공과_분리한다() {
    let (site, ram, renderer, count) = fixture("PUYO2.EXE");
    let raw_events = events(
        vec![
            lookup(&site, 0, OBJECT_RUNTIME),
            lookup(&site, 1, OBJECT_RUNTIME),
            primitive(&site, 1, OBJECT_RUNTIME),
        ],
        0,
    );
    let report = inspect(&site, &ram, &renderer, count, &raw_events).unwrap();
    assert_eq!(report.lookup_hits, 2);
    assert_eq!(report.primitive_hits, 1);
    assert_eq!(report.clipped_hits, 1);
    assert_eq!(report.rows[0].clipped_hits, 1);
}

#[test]
fn lookup과_primitive의_caller가_다르면_거부한다() {
    let (site, ram, renderer, count) = fixture("PUYO2.EXE");
    let raw_events = events(
        vec![
            lookup(&site, 0, OBJECT_RUNTIME),
            primitive_with_return(&site, 0, OBJECT_RUNTIME, RETURN_RUNTIME + 4),
        ],
        0,
    );
    assert!(inspect(&site, &ram, &renderer, count, &raw_events).is_err());
}

#[test]
fn dropped_event와_원본불일치와_고아primitive를_거부한다() {
    let (site, ram, renderer, count) = fixture("PUYO2.EXE");
    assert!(inspect(
        &site,
        &ram,
        &renderer,
        count,
        &events(vec![lookup(&site, 0, OBJECT_RUNTIME)], 1),
    )
    .is_err());

    let mut wrong_ram = ram.clone();
    let offset = psx_main_ram_offset(site.function_runtime, 1).unwrap();
    wrong_ram[offset] ^= 0xFF;
    assert!(inspect(
        &site,
        &wrong_ram,
        &renderer,
        count,
        &events(vec![lookup(&site, 0, OBJECT_RUNTIME)], 0),
    )
    .is_err());

    assert!(inspect(
        &site,
        &ram,
        &renderer,
        count,
        &events(vec![primitive(&site, 0, OBJECT_RUNTIME)], 0),
    )
    .is_err());
}
