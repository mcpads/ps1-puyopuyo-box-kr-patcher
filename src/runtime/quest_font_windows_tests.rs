use super::*;

const HEADER_LEN: usize = 32;

fn runtime_offset(runtime: u32, bytes: usize) -> usize {
    psx_main_ram_offset(runtime, bytes).expect("fixture RAM 범위")
}

fn put_u8(ram: &mut [u8], runtime: u32, value: u8) {
    ram[runtime_offset(runtime, 1)] = value;
}

fn put_u16(ram: &mut [u8], runtime: u32, value: u16) {
    let offset = runtime_offset(runtime, 2);
    ram[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(ram: &mut [u8], runtime: u32, value: u32) {
    let offset = runtime_offset(runtime, 4);
    ram[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn ram_fixture() -> Vec<u8> {
    let mut ram = vec![0u8; PSX_MAIN_RAM_LEN];
    for index in 0..QUEST_FONT_STATE_RECORD_COUNT {
        let state = QUEST_FONT_STATE_BASE_RUNTIME
            + u32::try_from(index * QUEST_FONT_STATE_RECORD_STRIDE).expect("작은 state");
        put_u8(
            &mut ram,
            state + u32::try_from(QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET).unwrap(),
            1,
        );
    }

    let state = QUEST_FONT_STATE_BASE_RUNTIME + 2 * QUEST_FONT_STATE_RECORD_STRIDE as u32;
    let parent = 0x800B_EC38;
    put_u32(&mut ram, state, parent);
    put_u8(&mut ram, state + 0x1A, 2);
    put_u8(&mut ram, state + 0x1B, 9);
    put_u8(&mut ram, state + QUEST_FONT_WINDOW_ACTIVE_OFFSET, 1);
    put_u16(&mut ram, state + QUEST_FONT_WINDOW_ORIGIN_X_OFFSET, 4);
    put_u16(&mut ram, state + QUEST_FONT_WINDOW_ORIGIN_Y_OFFSET, 8);
    put_u16(&mut ram, state + QUEST_FONT_WINDOW_CURSOR_X_OFFSET, 28);
    put_u16(&mut ram, state + QUEST_FONT_WINDOW_CURSOR_Y_OFFSET, 40);
    put_u8(&mut ram, state + QUEST_FONT_WINDOW_CELL_WIDTH_OFFSET, 12);
    put_u8(&mut ram, state + QUEST_FONT_WINDOW_CELL_HEIGHT_OFFSET, 16);
    put_u8(&mut ram, state + QUEST_FONT_WINDOW_LINE_HEIGHT_OFFSET, 16);
    put_u8(
        &mut ram,
        state + QUEST_FONT_WINDOW_GLYPH_SPACING_OFFSET,
        (-1i8) as u8,
    );
    put_u8(&mut ram, state + QUEST_FONT_WINDOW_LINE_SPACING_OFFSET, 2);
    put_u32(
        &mut ram,
        state + QUEST_FONT_WINDOW_INPUT_INDICATOR_OFFSET,
        0x800C_1000,
    );
    put_u32(
        &mut ram,
        state + QUEST_FONT_WINDOW_SELECTION_RENDERER_OFFSET,
        0x800C_2000,
    );
    put_u32(&mut ram, parent + 0x4C, (16i32 << 12) as u32);
    put_u32(&mut ram, parent + 0x50, (160i32 << 12) as u32);
    put_u32(&mut ram, parent + 0x58, 224);
    put_u32(&mut ram, parent + 0x5C, 64);
    ram
}

fn variable(name: &str, value: &[u8]) -> Vec<u8> {
    let mut raw = Vec::new();
    raw.push(u8::try_from(name.len()).expect("짧은 이름"));
    raw.extend_from_slice(name.as_bytes());
    raw.extend_from_slice(&u32::try_from(value.len()).expect("작은 값").to_le_bytes());
    raw.extend_from_slice(value);
    raw
}

fn section(name: &str, data: &[u8]) -> Vec<u8> {
    let mut raw = vec![0u8; 32];
    raw[..name.len()].copy_from_slice(name.as_bytes());
    raw.extend_from_slice(&u32::try_from(data.len()).expect("작은 값").to_le_bytes());
    raw.extend_from_slice(data);
    raw
}

fn state_fixture(ram: &[u8]) -> Vec<u8> {
    let main = variable("MainRAM.data8", ram);
    let mut raw = vec![0u8; HEADER_LEN];
    raw[..8].copy_from_slice(b"MDFNSVST");
    raw[16..20].copy_from_slice(&0x0010_3201u32.to_le_bytes());
    raw.extend_from_slice(&section("MAIN", &main));
    let len = u32::try_from(raw.len()).expect("작은 fixture");
    raw[20..24].copy_from_slice(&len.to_le_bytes());
    raw
}

#[test]
fn raw_ram에서_parent_geometry와_text_state를_함께_읽는다() {
    let census = inspect_quest_font_windows_ram(&ram_fixture()).unwrap();
    assert_eq!(census.slots.len(), QUEST_FONT_STATE_RECORD_COUNT);
    assert_eq!(census.active_windows, 1);
    assert_eq!(census.selection_renderer_slots, 1);
    assert_eq!(census.geometry_slots, 1);

    let slot = &census.slots[2];
    assert_eq!(slot.window_resource, 2);
    assert_eq!(slot.window_cell, 9);
    assert_eq!(slot.glyph_spacing, -1);
    assert_eq!(slot.line_spacing, 2);
    assert_eq!(
        slot.geometry,
        Some(QuestFontWindowGeometry {
            parent_x_fixed12: 16 << 12,
            parent_y_fixed12: 160 << 12,
            parent_x_px: 16,
            parent_y_px: 160,
            parent_width_px: 224,
            parent_height_px: 64,
            frame_origin_x_px: 8,
            frame_origin_y_px: 152,
        })
    );
}

#[test]
fn raw_ram과_save_state는_같은_census를_만든다() {
    let ram = ram_fixture();
    let raw_state = state_fixture(&ram);
    let state = MednafenPsxState::parse(&raw_state).unwrap();
    assert_eq!(
        inspect_quest_font_windows_ram(&ram).unwrap(),
        inspect_quest_font_windows(&state).unwrap()
    );
}

#[test]
fn 잘못된_ram_길이는_부분_census없이_거부한다() {
    let error = inspect_quest_font_windows_ram(&vec![0; PSX_MAIN_RAM_LEN - 1]).unwrap_err();
    assert!(matches!(
        error,
        QuestFontWindowCensusError::WrongRamLength { .. }
    ));
}

#[test]
fn tsv는_관측값을_digest에_귀속하고_미연결_parent를_구분한다() {
    let census = inspect_quest_font_windows_ram(&ram_fixture()).unwrap();
    let report = render_quest_font_window_geometry_tsv(&census, &[0x11; 32], &[0x22; 32]);
    assert!(report.contains(&format!(
        "quest-font-window-geometry-v1\t{}\t{}\t2",
        "11".repeat(32),
        "22".repeat(32)
    )));
    assert!(report.contains("\tobserved-parent\t1\t1\t2\t9\t"));
    assert!(report.contains("\tparent-not-bound\t0\t1\t0\t0\t"));
}
