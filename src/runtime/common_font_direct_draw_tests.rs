use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::*;

const DESCRIPTOR_ROOT_RUNTIME: u32 = 0x8008_C8F8;
const DESCRIPTOR_RUNTIME: u32 = DESCRIPTOR_ROOT_RUNTIME + 0x05A0;
const DESCRIPTOR_ROOT_POINTER_RUNTIME: u32 = 0x800D_B328;
const TABLE_RUNTIME: u32 = 0x8008_3A74;
const OBJECT_RUNTIME: u32 = 0x800D_0358;
const CALLBACK_RUNTIME: u32 = 0x8004_7450;
const WRITER_RUNTIME: u32 = 0x8004_CD28;
const WRITER_POST_RUNTIME: u32 = 0x8004_CD2C;
const HANDOFF_RUNTIME: u32 = 0x8004_7454;
const COMMIT_RUNTIME: u32 = 0x8004_755C;
const COMMAND_BUFFER_RUNTIME: u32 = 0x800E_FAC0;
const GP_RUNTIME: u32 = 0x800B_DF88;
const COMMAND_COUNT_RUNTIME: u32 = GP_RUNTIME + 0x021C;
const COMMAND_SLOT_RUNTIME: u32 = COMMAND_BUFFER_RUNTIME + 8;
const CELL: u16 = 27;
const RESOURCE: u16 = 40;
const CELL_COUNT: usize = 251;

struct Fixture {
    site: CommonFontDirectDrawSite,
    ram: Vec<u8>,
    writer: Vec<u8>,
    post_store: Vec<u8>,
    handoff: Vec<u8>,
    commit: Vec<u8>,
    screen: Vec<u8>,
}

fn write_u8(ram: &mut [u8], runtime: u32, value: u8) {
    let offset = psx_main_ram_offset(runtime, 1).unwrap();
    ram[offset] = value;
}

fn write_u16(ram: &mut [u8], runtime: u32, value: u16) {
    let offset = psx_main_ram_offset(runtime, 2).unwrap();
    ram[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(ram: &mut [u8], runtime: u32, value: u32) {
    let offset = psx_main_ram_offset(runtime, 4).unwrap();
    ram[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn event(pc: u32, registers: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"dropped": 0, "events": [{"pc": pc, "registers": registers}]}))
        .unwrap()
}

fn png() -> Vec<u8> {
    let mut raw = vec![0; 24];
    raw[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    raw[12..16].copy_from_slice(b"IHDR");
    raw[16..20].copy_from_slice(&350_u32.to_be_bytes());
    raw[20..24].copy_from_slice(&240_u32.to_be_bytes());
    raw
}

fn fixture() -> Fixture {
    let callback_source = [0x11, 0x22, 0x33, 0x44];
    let callback_source_sha256 = Sha256::digest(callback_source).into();
    let site = CommonFontDirectDrawSite {
        exe: "QUEST.EXE",
        archive: "/QUEST/RPG_GRP.VDD;1",
        surface_id: "quest-chain-label",
        source_text: "れんさ",
        writer_runtime: WRITER_RUNTIME,
        writer_post_runtime: WRITER_POST_RUNTIME,
        resource_store_runtime: WRITER_RUNTIME - 8,
        function_runtime: CALLBACK_RUNTIME,
        handoff_runtime: HANDOFF_RUNTIME,
        commit_runtime: COMMIT_RUNTIME,
        source_bytes: callback_source.len(),
        source_sha256: callback_source_sha256,
        descriptor_runtime: DESCRIPTOR_RUNTIME,
        descriptor_root_pointer_runtime: DESCRIPTOR_ROOT_POINTER_RUNTIME,
        descriptor_table_pointer_offset: 0x05BC,
        command_buffer_runtime: COMMAND_BUFFER_RUNTIME,
        command_count_gp_offset: 0x021C,
        cell_field_offset: 0x0066,
        resource_field_offset: 0x0068,
        callback_field_offset: 0x0074,
        cell: CELL,
        resource: RESOURCE,
        cell_count: CELL_COUNT,
    };

    let mut ram = vec![0; PSX_MAIN_RAM_LEN];
    let callback_offset = psx_main_ram_offset(CALLBACK_RUNTIME, callback_source.len()).unwrap();
    ram[callback_offset..callback_offset + callback_source.len()].copy_from_slice(&callback_source);
    write_u32(
        &mut ram,
        DESCRIPTOR_ROOT_POINTER_RUNTIME,
        DESCRIPTOR_ROOT_RUNTIME,
    );
    write_u32(&mut ram, DESCRIPTOR_RUNTIME, 1);
    write_u32(&mut ram, DESCRIPTOR_RUNTIME + 28, TABLE_RUNTIME);
    write_u16(&mut ram, TABLE_RUNTIME + 4, 256);
    write_u16(&mut ram, TABLE_RUNTIME + 6, 128);
    for index in 0..CELL_COUNT {
        let record = TABLE_RUNTIME + 12 + u32::try_from(index * 12).unwrap();
        write_u16(&mut ram, record + 4, 8);
        write_u16(&mut ram, record + 6, 8);
    }
    let target_record = TABLE_RUNTIME + 12 + u32::from(CELL) * 12;
    write_u16(&mut ram, target_record, 216);
    write_u16(&mut ram, target_record + 4, 24);
    write_u16(&mut ram, OBJECT_RUNTIME + 0x66, CELL);
    write_u16(&mut ram, OBJECT_RUNTIME + 0x68, RESOURCE);
    write_u8(&mut ram, OBJECT_RUNTIME + 0x6E, 1);
    write_u32(&mut ram, OBJECT_RUNTIME + 0x74, CALLBACK_RUNTIME);
    write_u32(&mut ram, COMMAND_COUNT_RUNTIME, 1);
    write_u16(&mut ram, COMMAND_SLOT_RUNTIME, 72);
    write_u16(&mut ram, COMMAND_SLOT_RUNTIME + 2, 205);
    write_u16(&mut ram, COMMAND_SLOT_RUNTIME + 4, CELL);
    write_u8(&mut ram, COMMAND_SLOT_RUNTIME + 6, 0);
    write_u8(&mut ram, COMMAND_SLOT_RUNTIME + 7, 1);

    Fixture {
        site,
        ram,
        writer: event(
            WRITER_RUNTIME,
            json!({"v0": CELL, "s2": RESOURCE, "a2": OBJECT_RUNTIME}),
        ),
        post_store: event(
            WRITER_POST_RUNTIME,
            json!({"v0": CELL, "s2": RESOURCE, "a2": OBJECT_RUNTIME}),
        ),
        handoff: event(
            HANDOFF_RUNTIME,
            json!({"a0": OBJECT_RUNTIME, "t0": OBJECT_RUNTIME}),
        ),
        commit: event(
            COMMIT_RUNTIME,
            json!({
                "t0": OBJECT_RUNTIME,
                "a1": CELL,
                "a2": target_record,
                "a3": 205,
                "a0": COMMAND_BUFFER_RUNTIME,
                "v0": 2,
                "v1": COMMAND_SLOT_RUNTIME,
                "gp": GP_RUNTIME
            }),
        ),
        screen: png(),
    }
}

fn inspect(fixture: &Fixture) -> Result<CommonFontDirectDrawReport, CommonFontDirectDrawError> {
    inspect_common_font_direct_draw(CommonFontDirectDrawObservationInput {
        site: &fixture.site,
        ram: &fixture.ram,
        writer_events_json: &fixture.writer,
        post_store_events_json: &fixture.post_store,
        handoff_events_json: &fixture.handoff,
        commit_events_json: &fixture.commit,
        screen_png: &fixture.screen,
    })
}

#[test]
fn writer에서_direct_command_commit까지_같은_object로_결속한다() {
    let fixture = fixture();
    let report = inspect(&fixture).expect("같은 object의 direct command는 결속해야 한다");

    assert_eq!(report.object_runtime, OBJECT_RUNTIME);
    assert_eq!(report.cell.index, usize::from(CELL));
    assert_eq!((report.cell.x, report.cell.width), (216, 24));
    assert_eq!(report.command_slot_runtime, COMMAND_SLOT_RUNTIME);
    assert_eq!((report.command_x, report.command_y), (72, 205));
    assert_eq!(report.command_cell, CELL);
    assert_eq!(report.command_flags, 1);
    assert!(report.object_fields_verified);
    assert!(report.descriptor_chain_verified);
    assert!(report.command_verified);
    assert!(report.runtime_verified);
    assert!(!report.final_pixel_attribution_verified);
    assert!(!report.release_eligible);

    let tsv = render_common_font_direct_draw_tsv(&report);
    assert!(tsv.contains("quest-chain-label\tれんさ"));
    assert!(tsv.contains("0x800D0358\t0x80083BC4\t40\t27\t216\t0\t24\t8"));
    assert!(tsv.contains("\t72\t205\t27\t0\t1\ttrue\ttrue\ttrue\ttrue\tfalse\ttrue\tfalse"));
}

#[test]
fn dropped_writer_event는_완전한_draw증거로_승격하지_않는다() {
    let mut fixture = fixture();
    fixture.writer = serde_json::to_vec(&json!({"dropped": 1, "events": []})).unwrap();

    let error = inspect(&fixture).unwrap_err();
    assert!(error.to_string().contains("writer dropped=1"));
}

#[test]
fn callback에_다른_object가_전달되면_거부한다() {
    let mut fixture = fixture();
    fixture.handoff = event(
        HANDOFF_RUNTIME,
        json!({"a0": OBJECT_RUNTIME + 4, "t0": OBJECT_RUNTIME + 4}),
    );

    let error = inspect(&fixture).unwrap_err();
    assert!(error
        .to_string()
        .contains("callback object argument 불일치"));
}

#[test]
fn command_slot의_cell이_object와_다르면_거부한다() {
    let mut fixture = fixture();
    write_u16(&mut fixture.ram, COMMAND_SLOT_RUNTIME + 4, CELL + 1);

    let error = inspect(&fixture).unwrap_err();
    assert!(error.to_string().contains("command cell/reserved 불일치"));
}

#[test]
fn commit이_다른_command_slot을_가리키면_거부한다() {
    let mut fixture = fixture();
    fixture.commit = event(
        COMMIT_RUNTIME,
        json!({
            "t0": OBJECT_RUNTIME,
            "a1": CELL,
            "a2": TABLE_RUNTIME + 12 + u32::from(CELL) * 12,
            "a3": 205,
            "a0": COMMAND_BUFFER_RUNTIME,
            "v0": 2,
            "v1": COMMAND_SLOT_RUNTIME + 8,
            "gp": GP_RUNTIME
        }),
    );

    let error = inspect(&fixture).unwrap_err();
    assert!(error.to_string().contains("commit command slot 불일치"));
}
